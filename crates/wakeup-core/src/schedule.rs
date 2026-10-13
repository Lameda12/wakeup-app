//! "Only watch me 9-6 on weekdays."

/// Active hours, stored readably in config.json:
/// `{ "start": "09:00", "end": "18:00", "days": [1, 2, 3, 4, 5] }`.
/// Days are ISO weekdays (1 = Monday ... 7 = Sunday). An end at or before
/// the start runs overnight, counted as part of the day it started.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ActiveHours {
    pub start: String,
    pub end: String,
    pub days: Vec<u8>,
}

impl Default for ActiveHours {
    fn default() -> Self {
        Self {
            start: "09:00".into(),
            end: "18:00".into(),
            days: vec![1, 2, 3, 4, 5],
        }
    }
}

/// Parses "HH:MM" into minutes after midnight.
pub fn parse_hhmm(s: &str) -> Option<u16> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m): (u16, u16) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

impl ActiveHours {
    pub fn is_valid(&self) -> bool {
        parse_hhmm(&self.start).is_some()
            && parse_hhmm(&self.end).is_some()
            && self.days.iter().all(|d| (1..=7).contains(d))
    }

    /// Is `minute` (after midnight) on ISO `weekday` inside the active hours?
    /// Invalid settings count as always active, so a typo never silently
    /// turns the app off.
    pub fn contains(&self, weekday: u8, minute: u16) -> bool {
        let (Some(start), Some(end)) = (parse_hhmm(&self.start), parse_hhmm(&self.end)) else {
            return true;
        };
        let day = |d: u8| self.days.contains(&d);
        let yesterday = if weekday == 1 { 7 } else { weekday - 1 };
        if start < end {
            day(weekday) && (start..end).contains(&minute)
        } else {
            (day(weekday) && minute >= start) || (day(yesterday) && minute < end)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hours(start: &str, end: &str, days: &[u8]) -> ActiveHours {
        ActiveHours {
            start: start.into(),
            end: end.into(),
            days: days.to_vec(),
        }
    }

    #[test]
    fn parses_times() {
        assert_eq!(parse_hhmm("09:30"), Some(570));
        assert_eq!(parse_hhmm("0:00"), Some(0));
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("9am"), None);
    }

    #[test]
    fn weekday_office_hours() {
        let h = ActiveHours::default();
        assert!(h.contains(1, 9 * 60));
        assert!(h.contains(5, 17 * 60 + 59));
        assert!(!h.contains(5, 18 * 60), "end is exclusive");
        assert!(!h.contains(3, 8 * 60 + 59));
        assert!(!h.contains(6, 12 * 60), "saturday");
    }

    #[test]
    fn overnight_belongs_to_the_starting_day() {
        let h = hours("22:00", "02:00", &[5]); // friday night
        assert!(h.contains(5, 23 * 60));
        assert!(h.contains(6, 60), "1am saturday is still friday night");
        assert!(!h.contains(6, 23 * 60), "saturday night isn't listed");
        assert!(!h.contains(5, 60), "1am friday belongs to thursday night");
    }

    #[test]
    fn sunday_wraps_to_monday() {
        let h = hours("23:00", "01:00", &[7]);
        assert!(h.contains(1, 30));
    }

    #[test]
    fn invalid_settings_fail_open() {
        let h = hours("nine", "18:00", &[1]);
        assert!(!h.is_valid());
        assert!(h.contains(6, 3 * 60));
        assert!(!hours("09:00", "18:00", &[0]).is_valid());
    }
}
