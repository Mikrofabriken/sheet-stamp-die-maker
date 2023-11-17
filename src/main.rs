use std::f32::consts::PI;

use image::io::Reader as ImageReader;
use image::{ImageBuffer, Luma};

const BLACK: u16 = 0;

const PIXELS_PER_MM: f32 = 10.0;
const MAX_FADE_DISTANCE: u32 = 45;
const SHEET_THICKNESS_PIXELS: u32 = 7;
const PUNCH_OUT_THICKNESS_MM: f32 = 2.0;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct PixelPosition {
    pub x: u32,
    pub y: u32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = ImageReader::open("../Paradise-rd.png")?.decode()?;
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
            let output_point = PixelPosition {
                x: output_x,
                y: output_y,
            };
            let distance_to_black = closest_black_pixel(&luma_img, output_point, MAX_FADE_DISTANCE)
                .unwrap_or(MAX_FADE_DISTANCE as f32);
            let output_color = fade_fn(distance_to_black);
            negative_form.put_pixel(output_x, output_y, Luma([output_color]));
        }
    }
    negative_form.save_with_format("../paradise-rd-negative.png", image::ImageFormat::Png)?;

    let mut positive_form: ImageBuffer<Luma<u16>, Vec<_>> = ImageBuffer::new(width, height);

    last_reported_percentage = 0;
    for positive_y in 0..height {
        let percentage = (positive_y as f32 / height as f32 * 100.0).floor() as u32;
        if percentage > last_reported_percentage {
            last_reported_percentage = percentage;
            println!("{percentage}%");
        }
        for positive_x in 0..width {
            let positive_point = PixelPosition {
                x: positive_x,
                y: positive_y,
            };
            let start_x = positive_x.saturating_sub(SHEET_THICKNESS_PIXELS);
            let end_x = positive_x.saturating_add(SHEET_THICKNESS_PIXELS).min(width);
            let start_y = positive_y.saturating_sub(SHEET_THICKNESS_PIXELS);
            let end_y = positive_y.saturating_add(SHEET_THICKNESS_PIXELS).min(height);

            let mut positive_z_mm = 0.0;
            for negative_y in start_y..end_y {
                for negative_x in start_x..end_x {
                    let negative_point = PixelPosition {
                        x: negative_x,
                        y: negative_y,
                    };
                    let xy_distance_mm = distance_pixels(positive_point, negative_point) / PIXELS_PER_MM;
                    if xy_distance_mm > SHEET_THICKNESS_PIXELS as f32 / PIXELS_PER_MM {
                        continue;
                    }
                    let negative_z_mm = negative_form.get_pixel(negative_x, negative_y).0[0] as f32
                        / u16::MAX as f32
                        * PUNCH_OUT_THICKNESS_MM;

                    let hypotenuse_mm = SHEET_THICKNESS_PIXELS as f32 / PIXELS_PER_MM;
                    let required_z_diff_mm = ((hypotenuse_mm * hypotenuse_mm) - (xy_distance_mm * xy_distance_mm)).sqrt();
                    let required_z = negative_z_mm + required_z_diff_mm;
                    if required_z > positive_z_mm {
                        positive_z_mm = required_z;
                    }
                }
            }
            positive_z_mm -= SHEET_THICKNESS_PIXELS as f32 / PIXELS_PER_MM;
            assert!(positive_z_mm >= 0.0);
            assert!(positive_z_mm <= PUNCH_OUT_THICKNESS_MM);
            let positive_pixel = ((positive_z_mm / PUNCH_OUT_THICKNESS_MM) * u16::MAX as f32) as u16;
            positive_form.put_pixel(positive_x, positive_y, Luma([positive_pixel]));
        }
    }
    positive_form.save_with_format("../paradise-rd-positive.png", image::ImageFormat::Png)?;

    Ok(())
}

/// Returns the distance from (x, y) to the closest pixel that is black, in `image`. Only searches the `max_distance` closest pixels
fn closest_black_pixel(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    point: PixelPosition,
    max_distance: u32,
) -> Option<f32> {
    let start_x = point.x.saturating_sub(max_distance);
    let end_x = point.x.saturating_add(max_distance).min(image.width());
    let start_y = point.y.saturating_sub(max_distance);
    let end_y = point.y.saturating_add(max_distance).min(image.height());
    let mut closest_location = None;
    for other_y in start_y..end_y {
        for other_x in start_x..end_x {
            if image.get_pixel(other_x, other_y).0[0] == BLACK {
                let distance = distance_pixels(
                    point,
                    PixelPosition {
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

fn distance_pixels(location1: PixelPosition, location2: PixelPosition) -> f32 {
    let dx = (location1.x as f32 - location2.x as f32).abs();
    let dy = (location1.y as f32 - location2.y as f32).abs();
    (dx * dx + dy * dy).sqrt()
}

fn fade_fn(distance_to_black: f32) -> u16 {
    let angle = (distance_to_black.min(MAX_FADE_DISTANCE as f32) / MAX_FADE_DISTANCE as f32) * PI;
    (((angle + PI).cos() + 1.0) / 2.0 * u16::MAX as f32) as u16
}
