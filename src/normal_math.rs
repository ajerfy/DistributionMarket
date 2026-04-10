use crate::fixed_point::Fixed;

/// A fixed-point Normal distribution shape intended to be stable to serialize and port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedNormalDistribution {
    pub mu: Fixed,
    pub sigma: Fixed,
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
    let old_lambda = fixed_calculate_lambda(from.sigma, k)?;
    let new_lambda = fixed_calculate_lambda(to.sigma, k)?;
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

    maximum_absolute_difference(from, old_lambda, to, new_lambda, lower, upper, 20_000)
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

fn maximum_absolute_difference(
    from: FixedNormalDistribution,
    from_lambda: Fixed,
    to: FixedNormalDistribution,
    to_lambda: Fixed,
    lower: Fixed,
    upper: Fixed,
    samples: usize,
) -> Result<Fixed, String> {
    let mut best = Fixed::ZERO;
    let range = upper - lower;
    let step_scale = Fixed::from_raw(samples as i128 * Fixed::SCALE);

    for step in 0..=samples {
        let x = lower + (range * Fixed::from_raw(step as i128 * Fixed::SCALE)) / step_scale;
        let old_value = fixed_calculate_value_from_lambda(x, from, from_lambda)?;
        let new_value = fixed_calculate_value_from_lambda(x, to, to_lambda)?;
        let value = (new_value - old_value).abs();
        if value.raw() > best.raw() {
            best = value;
        }
    }

    Ok(best)
}
