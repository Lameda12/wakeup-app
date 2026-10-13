use image::RgbImage;

/// A square, possibly rotated region of an image, in pixel coordinates.
///
/// `angle` is in radians and rotates the region's x axis clockwise in image
/// space (y points down), so a crop sampled with it comes out upright when
/// `angle` equals the face's roll.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Roi {
    pub cx: f32,
    pub cy: f32,
    pub size: f32,
    pub angle: f32,
}

impl Roi {
    /// Whole image, letterboxed into a square (what the detector expects).
    pub fn letterbox(width: u32, height: u32) -> Self {
        Self {
            cx: width as f32 / 2.0,
            cy: height as f32 / 2.0,
            size: width.max(height) as f32,
            angle: 0.0,
        }
    }

    /// Maps a point in normalized ROI space ([0,1] on both axes) to image pixels.
    pub fn to_image(&self, u: f32, v: f32) -> (f32, f32) {
        let (dx, dy) = ((u - 0.5) * self.size, (v - 0.5) * self.size);
        let (sin, cos) = self.angle.sin_cos();
        (self.cx + dx * cos - dy * sin, self.cy + dx * sin + dy * cos)
    }

    /// Samples the ROI into an `out`x`out` NHWC RGB tensor with bilinear
    /// filtering. Pixel values 0..=255 map linearly to `lo..=hi`; anything
    /// outside the image is treated as black.
    pub fn sample(&self, img: &RgbImage, out: u32, lo: f32, hi: f32) -> Vec<f32> {
        let scale = (hi - lo) / 255.0;
        let (w, h) = (img.width() as i64, img.height() as i64);
        let raw = img.as_raw();
        let px = |x: i64, y: i64, c: usize| -> f32 {
            if x < 0 || y < 0 || x >= w || y >= h {
                0.0
            } else {
                raw[((y * w + x) * 3) as usize + c] as f32
            }
        };

        let mut data = Vec::with_capacity((out * out * 3) as usize);
        for j in 0..out {
            for i in 0..out {
                let u = (i as f32 + 0.5) / out as f32;
                let v = (j as f32 + 0.5) / out as f32;
                let (x, y) = self.to_image(u, v);
                // Pixel centers sit at +0.5.
                let (x, y) = (x - 0.5, y - 0.5);
                let (x0, y0) = (x.floor() as i64, y.floor() as i64);
                let (fx, fy) = (x - x0 as f32, y - y0 as f32);
                for c in 0..3 {
                    let top = px(x0, y0, c) * (1.0 - fx) + px(x0 + 1, y0, c) * fx;
                    let bottom = px(x0, y0 + 1, c) * (1.0 - fx) + px(x0 + 1, y0 + 1, c) * fx;
                    data.push(lo + (top * (1.0 - fy) + bottom * fy) * scale);
                }
            }
        }
        data
    }
}

/// Builds the square ROI MediaPipe uses for the landmark model: bounding box
/// of `points`, rotated so the line from `a` to `b` is horizontal, enlarged by
/// `scale` along its longer side.
pub fn roi_around(points: &[(f32, f32)], a: (f32, f32), b: (f32, f32), scale: f32) -> Roi {
    let angle = (b.1 - a.1).atan2(b.0 - a.0);
    let (sin, cos) = angle.sin_cos();
    // Bounding box in the rotated frame, so a tilted face isn't over-cropped.
    let (mut min_u, mut max_u, mut min_v, mut max_v) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
    for &(x, y) in points {
        let u = x * cos + y * sin;
        let v = -x * sin + y * cos;
        min_u = min_u.min(u);
        max_u = max_u.max(u);
        min_v = min_v.min(v);
        max_v = max_v.max(v);
    }
    let (cu, cv) = ((min_u + max_u) / 2.0, (min_v + max_v) / 2.0);
    Roi {
        cx: cu * cos - cv * sin,
        cy: cu * sin + cv * cos,
        size: (max_u - min_u).max(max_v - min_v) * scale,
        angle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letterbox_maps_corners() {
        let roi = Roi::letterbox(640, 480);
        let (x, y) = roi.to_image(0.0, 0.0);
        assert!((x - 0.0).abs() < 1e-3 && (y - -80.0).abs() < 1e-3);
        let (x, y) = roi.to_image(1.0, 1.0);
        assert!((x - 640.0).abs() < 1e-3 && (y - 560.0).abs() < 1e-3);
    }

    #[test]
    fn rotated_roi_aligns_eye_line() {
        // Eyes on a 30 degree line: the ROI's x axis should follow it.
        let a = (100.0, 100.0);
        let b = (
            100.0 + 30f32.to_radians().cos() * 50.0,
            100.0 + 30f32.to_radians().sin() * 50.0,
        );
        let roi = roi_around(&[a, b], a, b, 1.0);
        assert!((roi.angle.to_degrees() - 30.0).abs() < 1e-3);
        let (x0, y0) = roi.to_image(0.0, 0.5);
        let (x1, y1) = roi.to_image(1.0, 0.5);
        assert!(((y1 - y0).atan2(x1 - x0).to_degrees() - 30.0).abs() < 1e-3);
    }

    #[test]
    fn sample_identity_and_padding() {
        let mut img = RgbImage::new(4, 4);
        img.put_pixel(1, 1, image::Rgb([255, 255, 255]));
        let roi = Roi {
            cx: 2.0,
            cy: 2.0,
            size: 4.0,
            angle: 0.0,
        };
        let t = roi.sample(&img, 4, 0.0, 1.0);
        // Pixel (1, 1) of a 4-wide image, red channel.
        assert_eq!(t[(4 + 1) * 3], 1.0);
        assert_eq!(t[0], 0.0);
        // Fully outside the image: black, i.e. `lo`.
        let off = Roi {
            cx: -100.0,
            cy: -100.0,
            size: 4.0,
            angle: 0.0,
        };
        assert!(off.sample(&img, 2, -1.0, 1.0).iter().all(|&v| v == -1.0));
    }
}
