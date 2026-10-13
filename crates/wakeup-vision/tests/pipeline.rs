//! End-to-end: real models on a real face. Needs ONNX Runtime, which the
//! default `download-binaries` feature provides.

use image::{RgbImage, imageops};
use wakeup_vision::{FacePipeline, FaceSample};

fn face() -> RgbImage {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/astronaut.jpg");
    image::open(path).expect("test image").to_rgb8()
}

/// Two passes, so the second uses the tracked crop like live video does.
fn analyze(img: &RgbImage) -> Option<FaceSample> {
    let mut p = FacePipeline::new().expect("models load");
    p.process(img).expect("first pass");
    p.process(img).expect("second pass").sample
}

#[test]
fn finds_open_eyes() {
    let s = analyze(&face()).expect("face found");
    assert!(s.presence > 0.9, "{s:?}");
    for ear in [s.ear_left, s.ear_right] {
        assert!(
            (0.22..0.45).contains(&ear),
            "open eyes should read 0.22-0.45, got {ear}"
        );
    }
}

#[test]
fn mirror_flips_roll_and_keeps_ear() {
    let img = face();
    let (a, b) = (
        analyze(&img).unwrap(),
        analyze(&imageops::flip_horizontal(&img)).unwrap(),
    );
    assert!(
        (a.roll_deg + b.roll_deg).abs() < 3.0,
        "{} vs {}",
        a.roll_deg,
        b.roll_deg
    );
    assert!((a.ear() - b.ear()).abs() < 0.03);
    assert!((a.pitch_deg - b.pitch_deg).abs() < 4.0);
}

/// Rotates clockwise by `deg` onto a canvas big enough to keep every pixel.
fn rotate(img: &RgbImage, deg: f32) -> RgbImage {
    let (w, h) = (img.width() as f32, img.height() as f32);
    let size = w.hypot(h).ceil() as u32;
    let (sin, cos) = (-deg.to_radians()).sin_cos();
    let (cx, cy, half) = (w / 2.0, h / 2.0, size as f32 / 2.0);
    RgbImage::from_fn(size, size, |x, y| {
        let (dx, dy) = (x as f32 - half, y as f32 - half);
        let (sx, sy) = (cx + dx * cos - dy * sin, cy + dx * sin + dy * cos);
        if sx >= 0.0 && sy >= 0.0 && sx < w && sy < h {
            *img.get_pixel(sx as u32, sy as u32)
        } else {
            image::Rgb([0, 0, 0])
        }
    })
}

#[test]
fn tracking_follows_a_dozing_head_tilt() {
    let img = face();
    let upright = analyze(&img).unwrap();
    // The first frame crops from the detector's coarse eye points; tracking
    // should converge on the true roll within a few frames.
    let tilted = rotate(&img, 45.0);
    let mut p = FacePipeline::new().unwrap();
    let mut last = None;
    for _ in 0..4 {
        last = p.process(&tilted).unwrap().sample;
    }
    let s = last.expect("face tracked at 45 degrees");
    assert!(
        (s.roll_deg - upright.roll_deg - 45.0).abs() < 5.0,
        "{} -> {}",
        upright.roll_deg,
        s.roll_deg
    );
    assert!((s.ear() - upright.ear()).abs() < 0.04);
}

#[test]
fn empty_frame_has_no_face() {
    let blank = RgbImage::from_pixel(640, 480, image::Rgb([40, 40, 40]));
    assert!(analyze(&blank).is_none());
}

#[test]
fn reacquires_after_face_leaves() {
    let mut p = FacePipeline::new().unwrap();
    let blank = RgbImage::from_pixel(384, 384, image::Rgb([40, 40, 40]));
    assert!(p.process(&face()).unwrap().sample.is_some());
    assert!(p.process(&blank).unwrap().sample.is_none());
    assert!(p.process(&blank).unwrap().sample.is_none());
    let back = p.process(&face()).unwrap();
    assert!(
        back.sample.is_some() && back.timings.detect.is_some(),
        "should re-detect"
    );
}
