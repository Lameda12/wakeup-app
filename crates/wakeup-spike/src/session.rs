//! Guided recording that answers M0's question: on this person, camera and
//! lighting, do open/closed eyes and neutral/down head pitch separate cleanly?

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::record::{Recorder, Row};
use crate::stats::{longest_run, percentile};
use crate::{CamArgs, cpu_time, run_camera};

/// Ignore the start of each phase while the person is still moving.
const SETTLE: f64 = 1.5;
/// The product rule we're validating: eyes closed this long fires the alarm.
const EYES_CLOSED_RULE: f64 = 2.5;

const PHASES: &[(&str, f64, &str)] = &[
    (
        "open",
        8.0,
        "Look at the screen like you're working. Eyes open.",
    ),
    (
        "closed",
        7.0,
        "Close your eyes. Keep them closed until you hear the next instruction.",
    ),
    ("open", 5.0, "Open your eyes. Look at the screen."),
    ("down", 7.0, "Look down at your phone, or at your lap."),
    ("open", 5.0, "Back to the screen."),
    ("blink", 6.0, "Blink a lot, fast."),
    ("away", 6.0, "Lean out of the camera's view."),
    ("open", 5.0, "Back in front of the screen. Last one."),
];

/// Speaks the instruction (macOS `say`), since one phase has eyes closed.
fn announce(text: &str) {
    println!("\n>>> {text}\n");
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("say").arg(text).spawn();
    #[cfg(not(target_os = "macos"))]
    print!("\x07");
}

fn phase_at(t: f64) -> Option<(usize, f64)> {
    let mut start = 0.0;
    for (i, &(_, len, _)) in PHASES.iter().enumerate() {
        if t < start + len {
            return Some((i, t - start));
        }
        start += len;
    }
    None
}

pub fn run(tag: &str, out: &Path, cam: CamArgs) -> Result<()> {
    let mut recorder = Recorder::create(out)?;
    let mut rows: Vec<Row> = Vec::new();

    println!(
        "wakeup M0 session \"{tag}\": about {:.0}s. Sit how you normally work.",
        PHASES.iter().map(|p| p.1).sum::<f64>()
    );
    announce("Starting in 3 seconds.");
    std::thread::sleep(Duration::from_secs(3));

    let (wall, cpu) = (Instant::now(), cpu_time());
    let mut current = usize::MAX;
    run_camera(cam, |mut row| {
        let Some((i, into)) = phase_at(row.t) else {
            return Ok(false);
        };
        if i != current {
            current = i;
            announce(PHASES[i].2);
        }
        row.phase = PHASES[i].0;
        row.tag = tag.to_string();
        recorder.write(&row)?;
        println!("{}", row.pretty());
        if into >= SETTLE {
            rows.push(row);
        }
        Ok(true)
    })?;
    let cpu_pct = (cpu_time() - cpu).as_secs_f64() / wall.elapsed().as_secs_f64() * 100.0;
    announce("Done.");

    report(&rows, cpu_pct);
    println!("\nraw data: {}", out.display());
    Ok(())
}

fn report(rows: &[Row], cpu_pct: f64) {
    let of = |phase: &str| rows.iter().filter(|r| r.phase == phase).collect::<Vec<_>>();
    let ears = |rs: &[&Row]| rs.iter().map(|r| r.ear()).collect::<Vec<_>>();
    let pitches = |rs: &[&Row]| rs.iter().map(|r| r.pitch).collect::<Vec<_>>();

    println!("\nphase    frames  face%   ear p10  ear med  ear p90   pitch med");
    for phase in ["open", "closed", "down", "blink", "away"] {
        let rs = of(phase);
        if rs.is_empty() {
            continue;
        }
        let face = rs.iter().filter(|r| r.face).count() as f32 / rs.len() as f32 * 100.0;
        let e = ears(&rs);
        println!(
            "{phase:<8} {:>6}  {face:>4.0}%   {:>7.3}  {:>7.3}  {:>7.3}   {:>+8.1}°",
            rs.len(),
            percentile(&e, 0.1),
            percentile(&e, 0.5),
            percentile(&e, 0.9),
            percentile(&pitches(&rs), 0.5)
        );
    }

    let (open, closed, down, blink, away) = (
        of("open"),
        of("closed"),
        of("down"),
        of("blink"),
        of("away"),
    );
    let mut ok = true;
    let mut verdict = |pass: bool, msg: String| {
        ok &= pass;
        println!("{} {msg}", if pass { "PASS" } else { "FAIL" });
    };
    println!();

    // Eyes: the 10th percentile open frame must sit above the 90th percentile closed frame.
    let (open_p10, closed_p90) = (
        percentile(&ears(&open), 0.1),
        percentile(&ears(&closed), 0.9),
    );
    let threshold = (percentile(&ears(&open), 0.5) + percentile(&ears(&closed), 0.5)) / 2.0;
    verdict(
        open_p10 > closed_p90,
        format!(
            "eyes: open p10 {open_p10:.3} vs closed p90 {closed_p90:.3}, suggested threshold {threshold:.3}"
        ),
    );

    let run_below = |rs: &[&Row]| {
        longest_run(
            &rs.iter()
                .map(|r| (r.t, r.face && r.ear() < threshold))
                .collect::<Vec<_>>(),
        )
    };
    let closed_run = run_below(&closed);
    let closed_len = PHASES.iter().find(|p| p.0 == "closed").map_or(0.0, |p| p.1) - SETTLE;
    verdict(
        closed_run >= EYES_CLOSED_RULE.min(closed_len - 0.5),
        format!(
            "eyes-closed rule: longest closed run {closed_run:.1}s while eyes were shut (rule needs {EYES_CLOSED_RULE}s)"
        ),
    );
    let blink_run = run_below(&blink).max(run_below(&open));
    verdict(
        blink_run < EYES_CLOSED_RULE / 2.0,
        format!("no false alarms: longest closed run while eyes open/blinking {blink_run:.1}s"),
    );

    // Head: looking down must move pitch well past normal jitter while working.
    let neutral = percentile(&pitches(&open), 0.5);
    let jitter = percentile(&pitches(&open), 0.9) - percentile(&pitches(&open), 0.1);
    let down_delta = percentile(&pitches(&down), 0.5) - neutral;
    let down_face = down.iter().filter(|r| r.face).count() as f32 / down.len().max(1) as f32;
    if down_delta.is_nan() || down_face < 0.5 {
        verdict(
            false,
            format!(
                "head down: face visible in only {:.0}% of frames; down needs the face-leaves-bottom rule",
                down_face * 100.0
            ),
        );
    } else {
        verdict(
            down_delta.abs() > (2.0 * jitter).max(10.0),
            format!(
                "head down: pitch moved {down_delta:+.1}° from neutral {neutral:+.1}° (normal jitter {jitter:.1}°)"
            ),
        );
        if down_delta < 0.0 {
            println!(
                "     note: pitch went negative looking down, so the sign is flipped on this camera. Tell Claude."
            );
        }
    }

    let away_face = away.iter().filter(|r| r.face).count() as f32 / away.len().max(1) as f32;
    verdict(
        away_face < 0.2,
        format!(
            "away: face seen in {:.0}% of frames after leaving",
            away_face * 100.0
        ),
    );

    let infer: Vec<f32> = rows.iter().map(|r| r.infer_ms() as f32).collect();
    println!(
        "\nperf: inference median {:.1}ms, p90 {:.1}ms; process CPU {cpu_pct:.0}% of one core (includes camera decode)",
        percentile(&infer, 0.5),
        percentile(&infer, 0.9)
    );
    println!(
        "\n{}",
        if ok {
            "M0 looks good on this setup."
        } else {
            "Some checks failed. Send this output + the CSV to Claude."
        }
    );

    // Ready to paste into the app's config.json (see README).
    let pitch_down = percentile(&pitches(&down), 0.5);
    if !threshold.is_nan() && !neutral.is_nan() && !pitch_down.is_nan() {
        println!(
            "\nfor the app's config.json:\n  \"ear_closed\": {threshold:.3},\n  \"pitch_neutral\": {neutral:.1},\n  \"pitch_down\": {pitch_down:.1}"
        );
    }
}
