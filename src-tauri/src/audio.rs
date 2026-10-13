//! Alarm siren and nudge chime, synthesized (no audio files to license or
//! bundle) and played on a dedicated thread, since audio output handles
//! aren't `Send` on every platform.

use std::f32::consts::TAU;
use std::num::NonZero;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

use rodio::{ChannelCount, DeviceSinkBuilder, Player, SampleRate, Source};

const RATE: u32 = 44_100;

enum Cmd {
    Alarm,
    Chime,
    Stop,
}

/// Handle to the audio thread. Cheap to clone.
#[derive(Clone)]
pub struct Audio(Sender<Cmd>);

impl Audio {
    pub fn spawn() -> Self {
        let (tx, rx) = channel();
        std::thread::Builder::new()
            .name("audio".into())
            .spawn(move || run(rx))
            .expect("spawn audio thread");
        Self(tx)
    }

    pub fn alarm(&self) {
        let _ = self.0.send(Cmd::Alarm);
    }

    pub fn chime(&self) {
        let _ = self.0.send(Cmd::Chime);
    }

    pub fn stop(&self) {
        let _ = self.0.send(Cmd::Stop);
    }
}

fn run(rx: Receiver<Cmd>) {
    let sink = match DeviceSinkBuilder::open_default_sink() {
        Ok(sink) => sink,
        Err(e) => {
            eprintln!("[audio] no output device, alarms will be silent: {e}");
            // Keep draining so senders never block or error.
            while rx.recv().is_ok() {}
            return;
        }
    };
    let alarm = Player::connect_new(sink.mixer());
    let chime = Player::connect_new(sink.mixer());
    while let Ok(cmd) = rx.recv() {
        match cmd {
            Cmd::Alarm => {
                alarm.clear();
                alarm.append(Siren::default());
                alarm.play();
            }
            Cmd::Chime => {
                chime.append(Chime::default());
                chime.play();
            }
            Cmd::Stop => alarm.clear(),
        }
    }
}

/// Envelope that fades `x` in and out over `edge` seconds inside [start, end).
fn gate(t: f32, start: f32, end: f32, edge: f32) -> f32 {
    if t < start || t >= end {
        0.0
    } else {
        ((t - start) / edge).min((end - t) / edge).min(1.0)
    }
}

/// Five seconds of alert beeps at moderate level, then a harsh rising and
/// falling siren at full level, forever.
#[derive(Default)]
pub struct Siren {
    n: u64,
    phase: f32,
}

impl Siren {
    const ESCALATE_AT: f32 = 5.0;
}

impl Iterator for Siren {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let t = self.n as f32 / RATE as f32;
        self.n += 1;
        let (freq, amp) = if t < Self::ESCALATE_AT {
            // Beep-beep-beep each second: 880, 1320, 880 Hz.
            let c = t.fract();
            let beeps = [(0.0, 880.0), (0.2, 1320.0), (0.4, 880.0)];
            beeps
                .iter()
                .map(|&(at, f)| (f, gate(c, at, at + 0.15, 0.005)))
                .fold((880.0, 0.0f32), |acc, x| if x.1 > 0.0 { x } else { acc })
        } else {
            // Triangle sweep 650 -> 1500 -> 650 Hz every 0.9s.
            let c = ((t - Self::ESCALATE_AT) / 0.9).fract();
            let tri = 1.0 - (2.0 * c - 1.0).abs();
            let ramp = ((t - Self::ESCALATE_AT) / 0.05).min(1.0);
            (650.0 + 850.0 * tri, ramp)
        };
        self.phase = (self.phase + TAU * freq / RATE as f32) % TAU;
        // Soft-clipped sine: louder-sounding and harsher than a pure tone.
        let level = if t < Self::ESCALATE_AT { 0.55 } else { 0.95 };
        Some((3.0 * self.phase.sin()).tanh() * amp * level)
    }
}

impl Source for Siren {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        NonZero::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        NonZero::new(RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// Two soft descending notes for the "still here?" nudge.
#[derive(Default)]
pub struct Chime {
    n: u64,
}

impl Chime {
    const LEN: f32 = 0.7;
}

impl Iterator for Chime {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let t = self.n as f32 / RATE as f32;
        if t >= Self::LEN {
            return None;
        }
        self.n += 1;
        let note = |f: f32, start: f32| {
            if t < start {
                0.0
            } else {
                let dt = t - start;
                (TAU * f * dt).sin() * (-dt * 7.0).exp() * gate(dt, 0.0, 10.0, 0.004)
            }
        };
        Some(0.25 * (note(1318.5, 0.0) + note(987.8, 0.18)))
    }
}

impl Source for Chime {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        NonZero::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        NonZero::new(RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(Self::LEN))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn siren_escalates_and_never_clips() {
        let samples: Vec<f32> = Siren::default().take(RATE as usize * 8).collect();
        assert!(samples.iter().all(|s| s.abs() <= 1.0));
        let sec = |i: usize| &samples[i * RATE as usize..(i + 1) * RATE as usize];
        let (early, late) = (rms(sec(1)), rms(sec(6)));
        assert!(early > 0.1, "beeps are audible: {early}");
        assert!(
            late > early * 1.5,
            "siren is much louder than the beeps: {early} -> {late}"
        );
    }

    #[test]
    fn beeps_have_gaps() {
        // 0.6-1.0s of each second is silent.
        let samples: Vec<f32> = Siren::default().take(RATE as usize).collect();
        let quiet = &samples[(RATE as f32 * 0.6) as usize..];
        assert!(quiet.iter().all(|s| s.abs() < 1e-6));
    }

    #[test]
    fn chime_is_short_and_soft() {
        let samples: Vec<f32> = Chime::default().collect();
        assert_eq!(samples.len(), (RATE as f32 * Chime::LEN).ceil() as usize);
        assert!(samples.iter().all(|s| s.abs() <= 0.5));
        assert!(rms(&samples) > 0.01);
    }
}
