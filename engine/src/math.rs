/// Clamp a f64 value between a minimum and maximum.
pub fn clamp_f64(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

/// Normalize a value to the range [0, 1] given its original range [min, max].
pub fn normalize_to_range(value: f64, min: f64, max: f64) -> f64 {
    if (max - min).abs() < f64::EPSILON {
        return 0.5;
    }
    let normalized = (value - min) / (max - min);
    clamp_f64(normalized, 0.0, 1.0)
}

/// Compute a weighted average of values given corresponding weights.
pub fn weighted_average(values: &[f64], weights: &[f64]) -> f64 {
    if values.is_empty() || values.len() != weights.len() {
        return 0.0;
    }
    let weight_sum: f64 = weights.iter().sum();
    if weight_sum.abs() < f64::EPSILON {
        return 0.0;
    }
    let weighted_sum: f64 = values.iter().zip(weights.iter()).map(|(v, w)| v * w).sum();
    weighted_sum / weight_sum
}

/// Compute the geometric mean of a slice of positive values.
pub fn geometric_mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let n = values.len() as f64;
    let log_sum: f64 = values
        .iter()
        .map(|v| {
            if *v <= 0.0 {
                f64::NEG_INFINITY
            } else {
                v.ln()
            }
        })
        .sum();
    if log_sum.is_infinite() {
        return 0.0;
    }
    (log_sum / n).exp()
}

/// Compute the variance of a slice of values.
pub fn compute_variance(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let sum_sq_diff: f64 = values.iter().map(|v| (v - mean).powi(2)).sum();
    sum_sq_diff / (n - 1.0)
}

/// Compute the standard deviation of a slice of values.
pub fn compute_std_dev(values: &[f64]) -> f64 {
    compute_variance(values).sqrt()
}

/// Compute the percentage difference between two values.
/// Returns a value in [0, inf) where 0 means equal.
pub fn percentage_difference(a: f64, b: f64) -> f64 {
    let denom = (a.abs() + b.abs()) / 2.0;
    if denom.abs() < f64::EPSILON {
        return 0.0;
    }
    ((a - b).abs() / denom) * 100.0
}

/// Convert basis points (1 bp = 0.01%) to a decimal multiplier.
/// E.g., 50 bps -> 0.005
pub fn basis_points_to_decimal(bps: u64) -> f64 {
    bps as f64 / 10_000.0
}

/// Convert a decimal to basis points.
/// E.g., 0.005 -> 50
pub fn decimal_to_basis_points(decimal: f64) -> u64 {
    (decimal * 10_000.0).round() as u64
}

/// Safely divide two f64 values, returning a default on division by zero.
pub fn safe_divide(numerator: f64, denominator: f64) -> f64 {
    if denominator.abs() < f64::EPSILON {
        0.0
    } else {
        numerator / denominator
    }
}

/// Linear interpolation between two values.
pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    let t_clamped = clamp_f64(t, 0.0, 1.0);
    a + (b - a) * t_clamped
}

/// Inverse linear interpolation: given a value between a and b, return t in [0, 1].
pub fn inverse_lerp(a: f64, b: f64, value: f64) -> f64 {
    if (b - a).abs() < f64::EPSILON {
        return 0.0;
    }
    clamp_f64((value - a) / (b - a), 0.0, 1.0)
}

/// Compute the exponential moving average given old value, new value, and alpha.
pub fn ema(old: f64, new: f64, alpha: f64) -> f64 {
    let a = clamp_f64(alpha, 0.0, 1.0);
    a * new + (1.0 - a) * old
}

/// Sigmoid function mapping any real to (0, 1).
pub fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// Softmax over a slice, returning a new Vec of probabilities.
pub fn softmax(values: &[f64]) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = values.iter().map(|v| (v - max_val).exp()).collect();
    let sum: f64 = exps.iter().sum();
    if sum.abs() < f64::EPSILON {
        return vec![1.0 / values.len() as f64; values.len()];
    }
    exps.iter().map(|e| e / sum).collect()
}

/// Harmonic mean of positive values.
pub fn harmonic_mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let reciprocal_sum: f64 = values
        .iter()
        .map(|v| {
            if v.abs() < f64::EPSILON {
                f64::INFINITY
            } else {
                1.0 / v
            }
        })
        .sum();
    if reciprocal_sum.is_infinite() || reciprocal_sum.abs() < f64::EPSILON {
        return 0.0;
    }
    values.len() as f64 / reciprocal_sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clamp() {
        assert_eq!(clamp_f64(5.0, 0.0, 10.0), 5.0);
        assert_eq!(clamp_f64(-1.0, 0.0, 10.0), 0.0);
        assert_eq!(clamp_f64(15.0, 0.0, 10.0), 10.0);
    }

    #[test]
    fn test_normalize_to_range() {
        assert!((normalize_to_range(5.0, 0.0, 10.0) - 0.5).abs() < 1e-9);
        assert!((normalize_to_range(0.0, 0.0, 10.0) - 0.0).abs() < 1e-9);
        assert!((normalize_to_range(10.0, 0.0, 10.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_weighted_average() {
        let vals = [1.0, 2.0, 3.0];
        let wts = [1.0, 1.0, 1.0];
        assert!((weighted_average(&vals, &wts) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn test_geometric_mean() {
        let vals = [2.0, 8.0];
        assert!((geometric_mean(&vals) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_variance_and_stddev() {
        let vals = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let var = compute_variance(&vals);
        assert!((var - 4.571).abs() < 0.01);
        let sd = compute_std_dev(&vals);
        assert!((sd - 2.138).abs() < 0.01);
    }

    #[test]
    fn test_basis_points() {
        assert!((basis_points_to_decimal(50) - 0.005).abs() < 1e-9);
        assert_eq!(decimal_to_basis_points(0.005), 50);
    }

    #[test]
    fn test_lerp_inverse_lerp() {
        assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < 1e-9);
        assert!((inverse_lerp(0.0, 10.0, 5.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_sigmoid() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_softmax_sums_to_one() {
        let vals = [1.0, 2.0, 3.0];
        let sm = softmax(&vals);
        let total: f64 = sm.iter().sum();
        assert!((total - 1.0).abs() < 1e-9);
    }
}
