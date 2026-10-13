//! Eye openness and head pose from face mesh landmarks.

use crate::mesh::Mesh;

/// Face mesh indices: [outer corner, top 1, top 2, inner corner, bottom 2, bottom 1].
/// "Right" is the subject's right eye, which appears on the left of the image.
pub const RIGHT_EYE: [usize; 6] = [33, 160, 158, 133, 153, 144];
pub const LEFT_EYE: [usize; 6] = [263, 387, 385, 362, 380, 373];
/// Outer eye corners, subject's right then left: the roll axis.
pub const EYE_LINE: [usize; 2] = [33, 263];
pub const FOREHEAD: usize = 10;
pub const CHIN: usize = 152;

/// What one camera frame says about the face, if there is one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceSample {
    /// Eye aspect ratio per eye; roughly 0.25-0.35 open, under ~0.15 closed.
    pub ear_left: f32,
    pub ear_right: f32,
    /// Head pitch in degrees, positive = chin down. Only meaningful relative
    /// to a calibrated neutral, since cameras sit at different heights.
    pub pitch_deg: f32,
    /// Head roll in degrees, positive = tilted clockwise in the image.
    pub roll_deg: f32,
    pub presence: f32,
    /// Face bounding box in pixels: min x, min y, max x, max y.
    pub bounds: [f32; 4],
}

impl FaceSample {
    pub fn ear(&self) -> f32 {
        (self.ear_left + self.ear_right) / 2.0
    }
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Eye aspect ratio (Soukupová & Čech 2016): mean eyelid gap over eye width.
pub fn ear(points: [(f32, f32); 6]) -> f32 {
    let [p1, p2, p3, p4, p5, p6] = points;
    let width = dist(p1, p4);
    if width <= f32::EPSILON {
        return 0.0;
    }
    (dist(p2, p6) + dist(p3, p5)) / (2.0 * width)
}

/// Pitch of the forehead-chin axis, positive when the chin tucks away from the
/// camera and the forehead comes toward it (looking down).
pub fn pitch_deg(forehead: [f32; 3], chin: [f32; 3]) -> f32 {
    (chin[2] - forehead[2])
        .atan2(chin[1] - forehead[1])
        .to_degrees()
}

/// Angle of the eye line in the image, positive = tilted clockwise.
pub fn roll_deg(right_eye: (f32, f32), left_eye: (f32, f32)) -> f32 {
    (left_eye.1 - right_eye.1)
        .atan2(left_eye.0 - right_eye.0)
        .to_degrees()
}

impl From<&Mesh> for FaceSample {
    fn from(mesh: &Mesh) -> Self {
        let eye = |idx: [usize; 6]| ear(idx.map(|i| mesh.xy(i)));
        Self {
            ear_left: eye(LEFT_EYE),
            ear_right: eye(RIGHT_EYE),
            pitch_deg: pitch_deg(mesh.points[FOREHEAD], mesh.points[CHIN]),
            roll_deg: roll_deg(mesh.xy(EYE_LINE[0]), mesh.xy(EYE_LINE[1])),
            presence: mesh.presence,
            bounds: mesh.bounds(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eye(gap: f32) -> [(f32, f32); 6] {
        [
            (0.0, 0.0),
            (3.0, -gap / 2.0),
            (7.0, -gap / 2.0),
            (10.0, 0.0),
            (7.0, gap / 2.0),
            (3.0, gap / 2.0),
        ]
    }

    #[test]
    fn ear_tracks_eyelid_gap() {
        assert!((ear(eye(3.0)) - 0.3).abs() < 1e-6);
        assert!((ear(eye(0.5)) - 0.05).abs() < 1e-6);
        assert_eq!(ear([(0.0, 0.0); 6]), 0.0);
    }

    #[test]
    fn ear_is_rotation_invariant() {
        let (sin, cos) = 0.4f32.sin_cos();
        let rotated = eye(3.0).map(|(x, y)| (x * cos - y * sin, x * sin + y * cos));
        assert!((ear(rotated) - 0.3).abs() < 1e-5);
    }

    #[test]
    fn pitch_sign() {
        // Upright: chin straight below forehead.
        assert!(pitch_deg([0.0, 0.0, 0.0], [0.0, 100.0, 0.0]).abs() < 1e-4);
        // Nodding down: forehead toward camera (smaller z), chin away.
        let down = 30f32.to_radians();
        let p = pitch_deg(
            [0.0, 0.0, -50.0 * down.sin()],
            [0.0, 100.0 * down.cos(), 50.0 * down.sin()],
        );
        assert!((p - 30.0).abs() < 1e-3, "{p}");
        assert!(pitch_deg([0.0, 0.0, 20.0], [0.0, 100.0, -20.0]) < 0.0);
    }
}
