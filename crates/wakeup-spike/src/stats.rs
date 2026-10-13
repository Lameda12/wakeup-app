/// Percentile (0..=1) of the non-NaN values, or NaN if there are none.
pub fn percentile(values: &[f32], q: f32) -> f32 {
    let mut v: Vec<f32> = values.iter().copied().filter(|x| !x.is_nan()).collect();
    if v.is_empty() {
        return f32::NAN;
    }
    v.sort_by(f32::total_cmp);
    v[((v.len() - 1) as f32 * q).round() as usize]
}

/// Longest stretch, in seconds, of consecutive `true` samples.
pub fn longest_run(samples: &[(f64, bool)]) -> f64 {
    let (mut best, mut start) = (0.0f64, None::<f64>);
    for &(t, hit) in samples {
        match (hit, start) {
            (true, None) => start = Some(t),
            (true, Some(s)) => best = best.max(t - s),
            (false, _) => start = None,
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_skip_nan() {
        let v = [3.0, f32::NAN, 1.0, 2.0];
        assert_eq!(percentile(&v, 0.0), 1.0);
        assert_eq!(percentile(&v, 0.5), 2.0);
        assert_eq!(percentile(&v, 1.0), 3.0);
        assert!(percentile(&[f32::NAN], 0.5).is_nan());
    }

    #[test]
    fn runs() {
        let s = [
            (0.0, true),
            (0.1, true),
            (0.2, false),
            (0.3, true),
            (0.4, true),
            (0.5, true),
            (0.6, false),
        ];
        assert!((longest_run(&s) - 0.2).abs() < 1e-9);
        assert_eq!(longest_run(&[]), 0.0);
    }
}
