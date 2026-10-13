use image::{Rgb, RgbImage};
use wakeup_vision::FrameResult;
use wakeup_vision::metrics::{CHIN, FOREHEAD, LEFT_EYE, RIGHT_EYE};

const GREEN: Rgb<u8> = Rgb([60, 220, 120]);
const RED: Rgb<u8> = Rgb([255, 59, 48]);
const YELLOW: Rgb<u8> = Rgb([255, 214, 10]);

fn dot(img: &mut RgbImage, x: f32, y: f32, r: i32, c: Rgb<u8>) {
    for dy in -r..=r {
        for dx in -r..=r {
            let (px, py) = (x as i32 + dx, y as i32 + dy);
            if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                img.put_pixel(px as u32, py as u32, c);
            }
        }
    }
}

/// Copy of `img` with the landmark crop, mesh points, eye points and the
/// pitch axis drawn on top.
pub fn draw(img: &RgbImage, result: &FrameResult) -> RgbImage {
    let mut out = img.clone();
    let Some(mesh) = &result.mesh else { return out };

    // Crop outline.
    let steps = 200;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        for (u, v) in [(t, 0.0), (t, 1.0), (0.0, t), (1.0, t)] {
            let (x, y) = mesh.roi.to_image(u, v);
            dot(&mut out, x, y, 0, YELLOW);
        }
    }
    for p in &mesh.points {
        dot(&mut out, p[0], p[1], 0, GREEN);
    }
    for i in RIGHT_EYE.iter().chain(&LEFT_EYE) {
        let (x, y) = mesh.xy(*i);
        dot(&mut out, x, y, 1, RED);
    }
    for i in [FOREHEAD, CHIN] {
        let (x, y) = mesh.xy(i);
        dot(&mut out, x, y, 2, YELLOW);
    }
    out
}
