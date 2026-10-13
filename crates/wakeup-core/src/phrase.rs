/// Dismiss phrases: long enough that you have to be awake to type them,
/// short enough not to be cruel.
const PHRASES: &[&str] = &[
    "i am awake and i will finish this",
    "my eyes are open and my phone is down",
    "back to work, no more scrolling",
    "i am awake and i know what i am doing",
    "one more hour of focus starts now",
    "i will not fall asleep on my keyboard",
];

/// Picks a phrase from any varying number (e.g. the alarm's start time), so
/// it changes between alarms without needing a random number generator.
pub fn pick_phrase(seed: u64) -> &'static str {
    PHRASES[(seed % PHRASES.len() as u64) as usize]
}

/// Case-insensitive, ignores punctuation and extra spaces. Typos still fail:
/// typing it right is the point.
pub fn matches_phrase(typed: &str, phrase: &str) -> bool {
    let norm = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    !phrase.is_empty() && norm(typed) == norm(phrase)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_matching() {
        let p = "back to work, no more scrolling";
        assert!(matches_phrase("back to work, no more scrolling", p));
        assert!(matches_phrase("  Back to WORK no more   scrolling. ", p));
        assert!(!matches_phrase("back to wrok, no more scrolling", p));
        assert!(!matches_phrase("back to work", p));
        assert!(!matches_phrase("", ""));
    }

    #[test]
    fn picks_vary() {
        assert_ne!(pick_phrase(0), pick_phrase(1));
        assert_eq!(pick_phrase(7), pick_phrase(7 + PHRASES.len() as u64));
    }
}
