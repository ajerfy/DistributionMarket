use crate::distributions::{ScaledDistribution, Support};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SearchRange {
    pub lower: f64,
    pub upper: f64,
}

impl SearchRange {
    pub fn new(lower: f64, upper: f64) -> Result<Self, String> {
        if !lower.is_finite() || !upper.is_finite() || upper <= lower {
            return Err("search range must be finite with upper > lower".to_string());
        }

        Ok(Self { lower, upper })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinimumResult {
    pub x_min: f64,
    pub value: f64,
}

pub fn position_difference(x: f64, old_f: &ScaledDistribution, new_f: &ScaledDistribution) -> f64 {
    new_f.value_at(x) - old_f.value_at(x)
}

pub fn find_global_minimum(
    old_f: &ScaledDistribution,
    new_f: &ScaledDistribution,
    search_range: SearchRange,
) -> Result<MinimumResult, String> {
    let adapted_range = adapted_search_range(old_f, new_f, search_range)?;
    let bracket = bracket_minimum(old_f, new_f, adapted_range);
    let refined = golden_section_minimum(old_f, new_f, bracket, 1e-10, 256);
    Ok(best_minimum(old_f, new_f, refined, adapted_range))
}

pub fn verify_minimum_onchain(
    x_min: f64,
    old_f: &ScaledDistribution,
    new_f: &ScaledDistribution,
) -> bool {
    let epsilon = 1e-5_f64.max(1e-4 * x_min.abs());
    let value = position_difference(x_min, old_f, new_f);
    let left = position_difference(x_min - epsilon, old_f, new_f);
    let right = position_difference(x_min + epsilon, old_f, new_f);
    let first_derivative = (right - left) / (2.0 * epsilon);
    let second_derivative = (right - 2.0 * value + left) / (epsilon * epsilon);

    first_derivative.abs() < 1e-4 && second_derivative >= -1e-7 && value <= left && value <= right
}

fn adapted_search_range(
    old_f: &ScaledDistribution,
    new_f: &ScaledDistribution,
    default_range: SearchRange,
) -> Result<SearchRange, String> {
    let old_location = old_f.mean_hint().unwrap_or(0.0);
    let new_location = new_f.mean_hint().unwrap_or(0.0);
    let old_scale = old_f.sigma_hint().unwrap_or(1.0);
    let new_scale = new_f.sigma_hint().unwrap_or(1.0);
    let mean_span = (old_location - new_location).abs();
    let scale_span = old_scale.max(new_scale);
    let tail_factor = old_f.search_tail_factor().max(new_f.search_tail_factor());

    let mut lower = old_location.min(new_location) - mean_span - tail_factor * scale_span;
    let mut upper = old_location.max(new_location) + mean_span + tail_factor * scale_span;

    if let Some(support) = merged_support(old_f.support_hint(), new_f.support_hint()) {
        lower = lower.min(support.lower - scale_span.max(1.0));
        upper = upper.max(support.upper + scale_span.max(1.0));
    }

    if lower.is_finite() && upper.is_finite() && upper > lower {
        SearchRange::new(lower, upper)
    } else {
        Ok(default_range)
    }
}

fn bracket_minimum(
    old_f: &ScaledDistribution,
    new_f: &ScaledDistribution,
    range: SearchRange,
) -> SearchRange {
    let samples = 512usize;
    let step = (range.upper - range.lower) / samples as f64;
    let mut best_index = 0usize;
    let mut best_value = f64::INFINITY;

    for index in 0..=samples {
        let x = range.lower + index as f64 * step;
        let value = position_difference(x, old_f, new_f);
        if value < best_value {
            best_value = value;
            best_index = index;
        }
    }

    let left_index = best_index.saturating_sub(1);
    let right_index = (best_index + 1).min(samples);
    let lower = range.lower + left_index as f64 * step;
    let upper = range.lower + right_index as f64 * step;

    SearchRange { lower, upper }
}

fn best_minimum(
    old_f: &ScaledDistribution,
    new_f: &ScaledDistribution,
    refined: MinimumResult,
    range: SearchRange,
) -> MinimumResult {
    let mut best = refined;

    for candidate in boundary_candidates(old_f, new_f, range) {
        let value = position_difference(candidate, old_f, new_f);
        if value < best.value {
            best = MinimumResult {
                x_min: candidate,
                value,
            };
        }
    }

    best
}

fn boundary_candidates(
    old_f: &ScaledDistribution,
    new_f: &ScaledDistribution,
    range: SearchRange,
) -> Vec<f64> {
    let mut candidates = vec![range.lower, range.upper];

    if let Some(support) = old_f.support_hint() {
        candidates.push(support.lower);
        candidates.push(support.upper);
    }

    if let Some(support) = new_f.support_hint() {
        candidates.push(support.lower);
        candidates.push(support.upper);
    }

    candidates.sort_by(|left, right| left.total_cmp(right));
    candidates.dedup_by(|left, right| (*left - *right).abs() < 1e-10);
    candidates
}

fn merged_support(left: Option<Support>, right: Option<Support>) -> Option<Support> {
    match (left, right) {
        (Some(left), Some(right)) => Some(Support {
            lower: left.lower.min(right.lower),
            upper: left.upper.max(right.upper),
        }),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

fn golden_section_minimum(
    old_f: &ScaledDistribution,
    new_f: &ScaledDistribution,
    range: SearchRange,
    tolerance: f64,
    max_iterations: usize,
) -> MinimumResult {
    let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let inv_phi = 1.0 / phi;

    let mut left = range.lower;
    let mut right = range.upper;
    let mut x1 = right - (right - left) * inv_phi;
    let mut x2 = left + (right - left) * inv_phi;
    let mut f1 = position_difference(x1, old_f, new_f);
    let mut f2 = position_difference(x2, old_f, new_f);

    for _ in 0..max_iterations {
        if (right - left).abs() <= tolerance {
            break;
        }

        if f1 < f2 {
            right = x2;
            x2 = x1;
            f2 = f1;
            x1 = right - (right - left) * inv_phi;
            f1 = position_difference(x1, old_f, new_f);
        } else {
            left = x1;
            x1 = x2;
            f1 = f2;
            x2 = left + (right - left) * inv_phi;
            f2 = position_difference(x2, old_f, new_f);
        }
    }

    let midpoint = (left + right) / 2.0;
    let midpoint_value = position_difference(midpoint, old_f, new_f);
    let left_value = position_difference(left, old_f, new_f);
    let right_value = position_difference(right, old_f, new_f);

    if left_value <= midpoint_value && left_value <= right_value {
        MinimumResult {
            x_min: left,
            value: left_value,
        }
    } else if right_value <= midpoint_value && right_value <= left_value {
        MinimumResult {
            x_min: right,
            value: right_value,
        }
    } else {
        MinimumResult {
            x_min: midpoint,
            value: midpoint_value,
        }
    }
}
