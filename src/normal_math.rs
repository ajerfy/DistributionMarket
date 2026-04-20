use crate::fixed_point::Fixed;

/// A fixed-point Normal distribution shape intended to be stable to serialize and port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedNormalDistribution {
    pub mu: Fixed,
    pub sigma: Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedSearchBounds {
    pub lower: Fixed,
    pub upper: Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedCollateralQuote {
    pub collateral_required: Fixed,
    pub lower_bound: Fixed,
    pub upper_bound: Fixed,
    pub coarse_samples: u32,
    pub refine_samples: u32,
}

impl FixedNormalDistribution {
    pub fn new(mu: Fixed, sigma: Fixed) -> Result<Self, String> {
        if sigma.raw() <= 0 {
            return Err("fixed normal sigma must be positive".to_string());
        }

        Ok(Self { mu, sigma })
    }
}

pub fn fixed_calculate_lambda(sigma: Fixed, k: Fixed) -> Result<Fixed, String> {
    let inner = Fixed::TWO * sigma * Fixed::SQRT_PI;
    Ok(k * inner.sqrt()?)
}

pub fn fixed_calculate_f(
    x: Fixed,
    distribution: FixedNormalDistribution,
    k: Fixed,
) -> Result<Fixed, String> {
    let lambda = fixed_calculate_lambda(distribution.sigma, k)?;
    fixed_calculate_value_from_lambda(x, distribution, lambda)
}

pub fn fixed_calculate_minimum_sigma(k: Fixed, b: Fixed) -> Result<Fixed, String> {
    Ok((k * k) / ((b * b) * Fixed::SQRT_PI))
}

pub fn fixed_calculate_maximum_k(sigma: Fixed, b: Fixed) -> Result<Fixed, String> {
    Ok(b * (sigma * Fixed::SQRT_PI).sqrt()?)
}

pub fn fixed_required_collateral(
    from: FixedNormalDistribution,
    to: FixedNormalDistribution,
    k: Fixed,
) -> Result<Fixed, String> {
    Ok(fixed_required_collateral_quote(from, to, k)?.collateral_required)
}

pub fn fixed_required_collateral_quote(
    from: FixedNormalDistribution,
    to: FixedNormalDistribution,
    k: Fixed,
) -> Result<FixedCollateralQuote, String> {
    let old_lambda = fixed_calculate_lambda(from.sigma, k)?;
    let new_lambda = fixed_calculate_lambda(to.sigma, k)?;
    let bounds = fixed_collateral_search_bounds(from, to);
    let coarse_samples = 4096_u32;
    let coarse = maximum_absolute_difference_with_argmax(
        from,
        old_lambda,
        to,
        new_lambda,
        bounds.lower,
        bounds.upper,
        coarse_samples as usize,
    )?;
    let coarse_step = (bounds.upper - bounds.lower).div_int(coarse_samples as i128);
    let refine_lower = (coarse.argmax - coarse_step).max(bounds.lower);
    let refine_upper = (coarse.argmax + coarse_step).min(bounds.upper);
    let refine_samples = 4096_u32;
    let refine = maximum_absolute_difference_with_argmax(
        from,
        old_lambda,
        to,
        new_lambda,
        refine_lower,
        refine_upper,
        refine_samples as usize,
    )?;

    let conservative_padding = Fixed::from_raw(1);
    let collateral_required = coarse
        .value
        .max(refine.value)
        .max(endpoint_maximum_absolute_difference(
            from,
            old_lambda,
            to,
            new_lambda,
            bounds.lower,
            bounds.upper,
        )?)
        + conservative_padding;

    Ok(FixedCollateralQuote {
        collateral_required,
        lower_bound: bounds.lower,
        upper_bound: bounds.upper,
        coarse_samples,
        refine_samples,
    })
}

pub fn fixed_calculate_value_from_lambda(
    x: Fixed,
    distribution: FixedNormalDistribution,
    lambda: Fixed,
) -> Result<Fixed, String> {
    let diff = x - distribution.mu;
    let diff_squared = diff * diff;
    let sigma_squared = distribution.sigma * distribution.sigma;
    let exponent = diff_squared / (Fixed::TWO * sigma_squared);
    let exp_term = exponent.exp_neg()?;
    let density = (Fixed::INV_SQRT_TWO_PI / distribution.sigma) * exp_term;
    Ok(lambda * density)
}

pub fn fixed_collateral_search_bounds(
    from: FixedNormalDistribution,
    to: FixedNormalDistribution,
) -> FixedSearchBounds {
    let span = (from.mu - to.mu).abs();
    let sigma = from.sigma.max(to.sigma);
    let tail = span + sigma.mul_int(8);

    let (lower, upper) = if from.mu.raw() < to.mu.raw() {
        (to.mu, to.mu + tail)
    } else if from.mu.raw() > to.mu.raw() {
        (to.mu - tail, to.mu)
    } else {
        (to.mu - sigma.mul_int(8), to.mu + sigma.mul_int(8))
    };

    FixedSearchBounds { lower, upper }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FixedArgmaxResult {
    argmax: Fixed,
    value: Fixed,
}

fn maximum_absolute_difference_with_argmax(
    from: FixedNormalDistribution,
    from_lambda: Fixed,
    to: FixedNormalDistribution,
    to_lambda: Fixed,
    lower: Fixed,
    upper: Fixed,
    samples: usize,
) -> Result<FixedArgmaxResult, String> {
    let mut best = Fixed::ZERO;
    let mut argmax = lower;
    let range = upper - lower;
    let step_scale = Fixed::from_raw(samples as i128 * Fixed::SCALE);

    for step in 0..=samples {
        let x = lower + (range * Fixed::from_raw(step as i128 * Fixed::SCALE)) / step_scale;
        let old_value = fixed_calculate_value_from_lambda(x, from, from_lambda)?;
        let new_value = fixed_calculate_value_from_lambda(x, to, to_lambda)?;
        let value = (new_value - old_value).abs();
        if value.raw() > best.raw() {
            best = value;
            argmax = x;
        }
    }

    Ok(FixedArgmaxResult { argmax, value: best })
}

fn endpoint_maximum_absolute_difference(
    from: FixedNormalDistribution,
    from_lambda: Fixed,
    to: FixedNormalDistribution,
    to_lambda: Fixed,
    lower: Fixed,
    upper: Fixed,
) -> Result<Fixed, String> {
    let lower_old = fixed_calculate_value_from_lambda(lower, from, from_lambda)?;
    let lower_new = fixed_calculate_value_from_lambda(lower, to, to_lambda)?;
    let upper_old = fixed_calculate_value_from_lambda(upper, from, from_lambda)?;
    let upper_new = fixed_calculate_value_from_lambda(upper, to, to_lambda)?;

    Ok((lower_new - lower_old)
        .abs()
        .max((upper_new - upper_old).abs()))
}
