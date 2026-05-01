//! Baseline statistics primitives.
//!
//! All functions take `&[f64]` (caller is responsible for filtering NaN /
//! converting from `SignalValue`). Empty inputs return `None` for `median`
//! and `mad`; `z_score` returns `0.0` when `mad == 0` (avoid divide-by-zero;
//! a flat baseline cannot be exceeded).

/// Median of a slice of `f64`. `None` for empty input.
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted: Vec<f64> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    if n % 2 == 1 {
        Some(sorted[n / 2])
    } else {
        Some((sorted[n / 2 - 1] + sorted[n / 2]) / 2.0)
    }
}

/// Median absolute deviation. `None` for empty input.
pub fn mad(values: &[f64]) -> Option<f64> {
    let m = median(values)?;
    let deviations: Vec<f64> = values.iter().map(|v| (v - m).abs()).collect();
    median(&deviations)
}

/// Modified z-score (per Iglewicz & Hoaglin, 1993). The 0.6745 factor
/// rescales MAD so the distribution matches the standard normal under
/// gaussian-like data.
pub fn z_score(value: f64, p50: f64, mad: f64) -> f64 {
    if mad == 0.0 {
        return 0.0;
    }
    0.6745 * (value - p50) / mad
}

/// Empirical percentile (linear interpolation). `p` in `[0.0, 100.0]`.
pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted: Vec<f64> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    if n == 1 {
        return Some(sorted[0]);
    }
    let rank = (p / 100.0) * (n as f64 - 1.0);
    let low = rank.floor() as usize;
    let high = rank.ceil() as usize;
    if low == high {
        Some(sorted[low])
    } else {
        let frac = rank - low as f64;
        Some(sorted[low] * (1.0 - frac) + sorted[high] * frac)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_p50_matches_median() {
        let v = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(&v, 50.0), median(&v));
    }

    #[test]
    fn percentile_p95_known() {
        let v: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        let p95 = percentile(&v, 95.0).unwrap();
        // For 1..=100 with linear interpolation: rank = 0.95 * 99 = 94.05;
        // sorted[94] = 95.0, sorted[95] = 96.0; result = 95 + 0.05 = 95.05
        assert!((p95 - 95.05).abs() < 0.01, "p95 = {}", p95);
    }

    #[test]
    fn percentile_empty_is_none() {
        assert_eq!(percentile(&[], 50.0), None);
    }

    #[test]
    fn percentile_single_value() {
        assert_eq!(percentile(&[42.0], 95.0), Some(42.0));
    }
}
