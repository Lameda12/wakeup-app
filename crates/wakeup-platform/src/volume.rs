//! Turns the output volume all the way up for the alarm and puts it back
//! afterwards. Best effort: if the OS refuses, the alarm still plays at
//! whatever volume is set.

use std::process::Command;

/// Volume as it was before the override, restored on drop.
#[derive(Debug)]
pub struct VolumeGuard {
    level: f32,
    muted: bool,
}

/// Saves the current volume, then unmutes and maxes it. `None` if the volume
/// can't be read on this system.
pub fn max_volume() -> Option<VolumeGuard> {
    let (level, muted) = imp::get()?;
    imp::set(1.0, false);
    Some(VolumeGuard { level, muted })
}

impl Drop for VolumeGuard {
    fn drop(&mut self) {
        imp::set(self.level, self.muted);
    }
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(target_os = "macos")]
mod imp {
    use super::run;

    pub fn get() -> Option<(f32, bool)> {
        let out = run(
            "osascript",
            &[
                "-e",
                "set s to get volume settings",
                "-e",
                "return (output volume of s as text) & \",\" & (output muted of s as text)",
            ],
        )?;
        super::parse_osascript(&out)
    }

    pub fn set(level: f32, muted: bool) {
        let pct = (level.clamp(0.0, 1.0) * 100.0).round() as u32;
        let mute = if muted { "with" } else { "without" };
        let _ = run(
            "osascript",
            &[
                "-e",
                &format!("set volume output volume {pct} {mute} output muted"),
            ],
        );
    }
}

/// PipeWire (default on current Ubuntu/Fedora/Arch). PulseAudio-only systems: M4.
#[cfg(target_os = "linux")]
mod imp {
    use super::run;

    const SINK: &str = "@DEFAULT_AUDIO_SINK@";

    pub fn get() -> Option<(f32, bool)> {
        super::parse_wpctl(&run("wpctl", &["get-volume", SINK])?)
    }

    pub fn set(level: f32, muted: bool) {
        let _ = run(
            "wpctl",
            &["set-volume", SINK, &format!("{:.2}", level.clamp(0.0, 1.0))],
        );
        let _ = run("wpctl", &["set-mute", SINK, if muted { "1" } else { "0" }]);
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod imp {
    pub fn get() -> Option<(f32, bool)> {
        None
    }

    pub fn set(_: f32, _: bool) {}
}

/// Parses `"40,false"` from the macOS volume settings script.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_osascript(out: &str) -> Option<(f32, bool)> {
    let (level, muted) = out.trim().split_once(',')?;
    Some((
        level.trim().parse::<f32>().ok()? / 100.0,
        muted.trim() == "true",
    ))
}

/// Parses `"Volume: 0.40"` or `"Volume: 0.40 [MUTED]"` from `wpctl get-volume`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_wpctl(out: &str) -> Option<(f32, bool)> {
    let rest = out.trim().strip_prefix("Volume:")?.trim();
    let level = rest.split_whitespace().next()?.parse().ok()?;
    Some((level, rest.contains("[MUTED]")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_macos_volume() {
        assert_eq!(parse_osascript("40,false\n"), Some((0.4, false)));
        assert_eq!(parse_osascript("0,true"), Some((0.0, true)));
        assert_eq!(parse_osascript("missing value,false"), None);
        assert_eq!(parse_osascript(""), None);
    }

    #[test]
    fn parses_wpctl_volume() {
        assert_eq!(parse_wpctl("Volume: 0.40\n"), Some((0.4, false)));
        assert_eq!(parse_wpctl("Volume: 1.00 [MUTED]"), Some((1.0, true)));
        assert_eq!(parse_wpctl("error"), None);
    }
}
