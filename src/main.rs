use std::f32::consts::PI;
use std::path::PathBuf;

use clap::Parser;
use image::io::Reader as ImageReader;
use image::{DynamicImage, ImageBuffer, Luma};

const BLACK: u16 = 0;

const PIXELS_PER_MM: f32 = 10.0;
const SHEET_THICKNESS_PIXELS: u32 = 7;

#[derive(clap::Parser, Debug)]
struct Args {
    /// Input image file to create stamp dies from. Should be black and white. White is where the sheet
    /// will be stamped out.
    input: PathBuf,

    /// How many millimeters deep to punch out. The height difference between white and black. (Z distance)
    #[arg(long)]
    punch_out_depth: f32,

    /// Over how many millimeters in the XY plane (along the sheet) to do the transition from black to white.
    /// A higher value provides a smoother curve for the sheet to bend along, but reduces details.
    #[arg(long, default_value_t = 4.5)]
    fade_distance: f32,
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
            let output_color = if let Some(distance_to_black_mm) =
                closest_black_pixel(&luma_img, output_coordinate, args.fade_distance)
            {
                fade_fn(distance_to_black_mm, args.fade_distance)
            } else {
                u16::MAX
            };
            negative_form.put_pixel(output_x, output_y, Luma([output_color]));
        }
    }

    let mut positive_form: ImageBuffer<Luma<u16>, Vec<_>> = ImageBuffer::new(width, height);

    last_reported_percentage = 0;
    for positive_y in 0..height {
        let percentage = (positive_y as f32 / height as f32 * 100.0).floor() as u32;
        if percentage > last_reported_percentage {
            last_reported_percentage = percentage;
            println!("{percentage}%");
        }
        for positive_x in 0..width {
            let positive_coordinate = PixelCoordinate {
                x: positive_x,
                y: positive_y,
            };
            let start_x = positive_x.saturating_sub(SHEET_THICKNESS_PIXELS);
            let end_x = positive_x.saturating_add(SHEET_THICKNESS_PIXELS).min(width);
            let start_y = positive_y.saturating_sub(SHEET_THICKNESS_PIXELS);
            let end_y = positive_y
                .saturating_add(SHEET_THICKNESS_PIXELS)
                .min(height);

            let mut positive_z_mm = 0.0;
            for negative_y in start_y..end_y {
                for negative_x in start_x..end_x {
                    let negative_coordinate = PixelCoordinate {
                        x: negative_x,
                        y: negative_y,
                    };
                    let xy_distance_mm = distance_mm(positive_coordinate, negative_coordinate);
                    if xy_distance_mm > SHEET_THICKNESS_PIXELS as f32 / PIXELS_PER_MM {
                        continue;
                    }
                    let negative_z_mm = negative_form.get_pixel(negative_x, negative_y).0[0] as f32
                        / u16::MAX as f32
                        * args.punch_out_depth;

                    let hypotenuse_mm = SHEET_THICKNESS_PIXELS as f32 / PIXELS_PER_MM;
                    let required_z_diff_mm = ((hypotenuse_mm * hypotenuse_mm)
                        - (xy_distance_mm * xy_distance_mm))
                        .sqrt();
                    let required_z = negative_z_mm + required_z_diff_mm;
                    if required_z > positive_z_mm {
                        positive_z_mm = required_z;
                    }
                }
            }
            positive_z_mm -= SHEET_THICKNESS_PIXELS as f32 / PIXELS_PER_MM;
            assert!(positive_z_mm >= 0.0);
            assert!(positive_z_mm <= args.punch_out_depth);
            let positive_pixel = ((positive_z_mm / args.punch_out_depth) * u16::MAX as f32) as u16;
            positive_form.put_pixel(positive_x, positive_y, Luma([positive_pixel]));
        }
    }
    let negative_form = DynamicImage::from(negative_form).fliph();
    negative_form.save_with_format("../paradise-rd-negative.png", image::ImageFormat::Png)?;
    let mut positive_form = DynamicImage::from(positive_form);
    positive_form.invert();
    positive_form.save_with_format("../paradise-rd-positive.png", image::ImageFormat::Png)?;

    Ok(())
}

/// Returns the distance (in mm) from `coordinate` to the closest pixel that is black, in `image`. Only searches the `max_distance` closest pixels
fn closest_black_pixel(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    coordinate: PixelCoordinate,
    max_distance_mm: f32,
) -> Option<f32> {
    let max_distance_pixels = (max_distance_mm * PIXELS_PER_MM).floor() as u32;
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

fn distance_mm(location1: PixelCoordinate, location2: PixelCoordinate) -> f32 {
    let dx = (location1.x as f32 - location2.x as f32) / PIXELS_PER_MM;
    let dy = (location1.y as f32 - location2.y as f32) / PIXELS_PER_MM;
    (dx * dx + dy * dy).sqrt()
}

fn fade_fn(distance_to_black_mm: f32, fade_distance_mm: f32) -> u16 {
    if distance_to_black_mm > fade_distance_mm {
        return u16::MAX;
    }
    let angle = (distance_to_black_mm / fade_distance_mm as f32) * PI;
    (((angle + PI).cos() + 1.0) / 2.0 * u16::MAX as f32) as u16
}
