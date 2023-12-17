use std::f32::consts::PI;
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::Parser;
use image::io::Reader as ImageReader;
use image::{DynamicImage, ImageBuffer, Luma};

mod neighbor_iterator;

const BLACK: u16 = 0;

#[derive(clap::Parser, Debug)]
struct Args {
    /// Input image file to create stamp dies from. Should be black and white. White is where the sheet
    /// will be stamped out.
    input: PathBuf,

    /// How many millimeters deep to punch out. The height difference between white and black. (Z distance)
    #[arg(long)]
    punch_out_depth: f32,

    /// How thick the sheet to stamp is (in millimeters).
    /// Determines the  distance between the positive and negative forms.
    #[arg(long, default_value_t = 0.7)]
    sheet_thickness: f32,

    /// Over how many millimeters in the XY plane (along the sheet) to do the transition from black to white.
    /// A higher value provides a smoother curve for the sheet to bend along, but reduces details.
    #[arg(long, default_value_t = 4.5)]
    fade_distance: f32,

    /// Resolution of the input image. Needed to convert between pixel coordinates and real world distance.
    /// The default value of 0.1 mm per pixel gives enough resolution for most practical use cases.
    #[arg(long, default_value_t = 10.0)]
    pixels_per_mm: f32,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct PixelCoordinate {
    pub x: u32,
    pub y: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let img = ImageReader::open(&args.input)?.decode()?;
    let luma_img = img.into_luma16();

    let (width, height) = luma_img.dimensions();
    println!("Hello, world! {width}x{height}");

    let mut negative_form: ImageBuffer<Luma<u16>, Vec<_>> = ImageBuffer::new(width, height);

    let negative_form_start = Instant::now();
    let mut last_reported_percentage = 0;
    for output_y in 0..height {
        let percentage = (output_y as f32 / height as f32 * 100.0).floor() as u32;
        if percentage > last_reported_percentage {
            last_reported_percentage = percentage;
            println!("{percentage}%");
        }
        for output_x in 0..width {
            let output_coordinate = PixelCoordinate {
                x: output_x,
                y: output_y,
            };
            let output_color = if let Some(distance_to_black_mm) = closest_black_pixel(
                &luma_img,
                output_coordinate,
                args.fade_distance,
                args.pixels_per_mm,
            ) {
                fade_fn(distance_to_black_mm, args.fade_distance)
            } else {
                u16::MAX
            };
            negative_form.put_pixel(output_x, output_y, Luma([output_color]));
        }
    }
    let negative_form_compute_time = negative_form_start.elapsed();
    println!(
        "Computing negative form took {} ms",
        negative_form_compute_time.as_millis()
    );

    let mut positive_form: ImageBuffer<Luma<u16>, Vec<_>> = ImageBuffer::new(width, height);
    let positive_form_start = Instant::now();
    let sheet_thickness_neighbors =
        neighbor_iterator::Neighbors::new(args.sheet_thickness * args.pixels_per_mm);

    last_reported_percentage = 0;
    for positive_y in 0..height {
        let percentage = (positive_y as f32 / height as f32 * 100.0).floor() as u32;
        if percentage > last_reported_percentage {
            last_reported_percentage = percentage;
            println!("{percentage}%");
        }
        for positive_x in 0..width {
            let mut positive_z_mm = 0.0;
            for (offset, distance_pixels) in &sheet_thickness_neighbors {
                let negative_y = positive_y as i32 + offset.y;
                let negative_x = positive_x as i32 + offset.x;
                // Skip pixels outside the image
                if negative_y < 0
                    || negative_y >= height as i32
                    || negative_x < 0
                    || negative_x >= width as i32
                {
                    continue;
                }

                // Safe to cast to u32 since we check for <0 above
                let negative_coordinate = PixelCoordinate {
                    x: negative_x as u32,
                    y: negative_y as u32,
                };
                let xy_distance_mm = distance_pixels / args.pixels_per_mm;
                let negative_z_mm = negative_form
                    .get_pixel(negative_coordinate.x, negative_coordinate.y)
                    .0[0] as f32
                    / u16::MAX as f32
                    * args.punch_out_depth;

                // Compute the missing side of the triangle. The sheet thickness is the hypotenuse
                // and the positive to negative xy-distance is one known side.
                let required_z_diff_mm = ((args.sheet_thickness * args.sheet_thickness)
                    - (xy_distance_mm * xy_distance_mm))
                    .sqrt();
                let required_z = negative_z_mm + required_z_diff_mm;
                // Bump up positive_z_mm if required_z is higher than currently held value
                if required_z > positive_z_mm {
                    positive_z_mm = required_z;
                }
                // Abort early if we are already so high up that subsequent pixels can't push us higher.
                // We can do this optimization since we know that `positive_z_mm` will only ever increase
                // and `required_z_diff_mm` will only shrink towards zero.
                if positive_z_mm > args.punch_out_depth + required_z_diff_mm {
                    break;
                }
            }
            positive_z_mm -= args.sheet_thickness;
            assert!(positive_z_mm >= 0.0);
            assert!(positive_z_mm <= args.punch_out_depth);
            let positive_pixel = ((positive_z_mm / args.punch_out_depth) * u16::MAX as f32) as u16;
            positive_form.put_pixel(positive_x, positive_y, Luma([positive_pixel]));
        }
    }
    let positive_form_compute_time = positive_form_start.elapsed();
    println!(
        "Computing positive form took {} ms",
        positive_form_compute_time.as_millis()
    );

    let negative_output_path =
        output_path(&args.input, "negative").expect("Unable to convert input path to output path");
    let negative_form = DynamicImage::from(negative_form).fliph();
    negative_form.save_with_format(negative_output_path, image::ImageFormat::Png)?;

    let positive_output_path =
        output_path(&args.input, "positive").expect("Unable to convert input path to output path");
    let mut positive_form = DynamicImage::from(positive_form);
    positive_form.invert();
    positive_form.save_with_format(positive_output_path, image::ImageFormat::Png)?;

    Ok(())
}

fn output_path(input_path: &Path, form_type: &str) -> Option<PathBuf> {
    let dir = input_path.parent()?;
    let mut filename = input_path.file_stem()?.to_owned();
    filename.push(format!(".{form_type}.png"));
    Some(dir.join(filename))
}

/// Returns the distance (in mm) from `coordinate` to the closest pixel that is black, in `image`. Only searches the `max_distance` closest pixels
fn closest_black_pixel(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    coordinate: PixelCoordinate,
    max_distance_mm: f32,
    pixels_per_mm: f32,
) -> Option<f32> {
    let max_distance_pixels = (max_distance_mm * pixels_per_mm).floor() as u32;
    let start_x = coordinate.x.saturating_sub(max_distance_pixels);
    let end_x = coordinate
        .x
        .saturating_add(max_distance_pixels)
        .min(image.width());
    let start_y = coordinate.y.saturating_sub(max_distance_pixels);
    let end_y = coordinate
        .y
        .saturating_add(max_distance_pixels)
        .min(image.height());
    let mut closest_location = None;
    for other_y in start_y..end_y {
        for other_x in start_x..end_x {
            if image.get_pixel(other_x, other_y).0[0] == BLACK {
                let distance = distance_mm(
                    coordinate,
                    PixelCoordinate {
                        x: other_x,
                        y: other_y,
                    },
                    pixels_per_mm,
                );
                if let Some(closest_location) = closest_location.as_mut() {
                    if distance < *closest_location {
                        *closest_location = distance;
                    }
                } else {
                    closest_location = Some(distance);
                }
            }
        }
    }
    closest_location
}

fn distance_mm(location1: PixelCoordinate, location2: PixelCoordinate, pixels_per_mm: f32) -> f32 {
    let dx = (location1.x as f32 - location2.x as f32) / pixels_per_mm;
    let dy = (location1.y as f32 - location2.y as f32) / pixels_per_mm;
    (dx * dx + dy * dy).sqrt()
}

fn fade_fn(distance_to_black_mm: f32, fade_distance_mm: f32) -> u16 {
    if distance_to_black_mm > fade_distance_mm {
        return u16::MAX;
    }
    let angle = (distance_to_black_mm / fade_distance_mm as f32) * PI;
    (((angle + PI).cos() + 1.0) / 2.0 * u16::MAX as f32) as u16
}
