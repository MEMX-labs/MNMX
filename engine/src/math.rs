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
