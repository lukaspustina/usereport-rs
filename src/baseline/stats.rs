//! Baseline statistics primitives.
//!
//! All functions take `&[f64]` (caller is responsible for filtering NaN /
//! converting from `SignalValue`). Empty inputs return `None` for `median`
//! and `mad`; `z_score` returns `0.0` when `mad == 0` (avoid divide-by-zero;
//! a flat baseline cannot be exceeded).

use crate::signal::{SampleStats, Trend};

/// Median of a slice of `f64`. `None` for empty input or all-non-finite input.
pub fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return None;
    }
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
/// rescales MAD to match the standard normal under Gaussian-like data.
/// Callers use `Z_WARN_THRESHOLD` (3.5) and `Z_CRIT_THRESHOLD` (7.0).
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
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return None;
    }
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

/// Compute `SampleStats` (min, max, p50, p95, trend) from a slice of f64
/// samples. Returns `None` for empty input. Trend is determined by linear
/// regression slope: |slope| < 5% of |p50| → Flat (with a floor of 0.01 when
/// p50 ≈ 0); positive slope → Rising; negative slope → Falling.
pub fn sample_stats(values: &[f64]) -> Option<SampleStats> {
    if values.is_empty() {
        return None;
    }
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let p50 = percentile(values, 50.0)?;
    let p95 = percentile(values, 95.0)?;
    let p99 = percentile(values, 99.0)?;
    let trend = linear_trend(values, p50);
    Some(SampleStats {
        min,
        max,
        p50,
        p95,
        p99,
        trend,
    })
}

fn linear_trend(values: &[f64], p50: f64) -> Trend {
    let n = values.len();
    if n < 2 {
        return Trend::Flat;
    }
    let n_f = n as f64;
    let mean_x = (n_f - 1.0) / 2.0;
    let mean_y = values.iter().sum::<f64>() / n_f;
    let mut num = 0.0f64;
    let mut den = 0.0f64;
    for (i, &v) in values.iter().enumerate() {
        let x = i as f64 - mean_x;
        num += x * (v - mean_y);
        den += x * x;
    }
    if den == 0.0 {
        return Trend::Flat;
    }
    let slope = num / den;
    let flat_threshold = (p50.abs() * 0.05).max(0.01);
    if slope.abs() < flat_threshold {
        Trend::Flat
    } else if slope > 0.0 {
        Trend::Rising
    } else {
        Trend::Falling
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

    #[test]
    fn mad_zero_when_all_same() {
        let v = [5.0, 5.0, 5.0, 5.0];
        assert_eq!(mad(&v), Some(0.0));
    }

    #[test]
    fn median_two_elements() {
        assert_eq!(median(&[3.0, 7.0]), Some(5.0));
    }

    #[test]
    fn z_score_with_mad_zero() {
        let z = z_score(100.0, 50.0, 0.0);
        assert_eq!(z, 0.0);
        assert!(!z.is_nan());
        assert!(!z.is_infinite());
    }

    #[test]
    fn percentile_boundary() {
        let v = [1.0, 2.0, 3.0, 4.0, 5.0];
        // p=0.0 → first element (min), p=100.0 → last element (max)
        assert_eq!(percentile(&v, 0.0), Some(1.0));
        assert_eq!(percentile(&v, 100.0), Some(5.0));
    }
}
