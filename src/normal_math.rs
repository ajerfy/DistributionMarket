use crate::distributions::{NormalDistribution, ScaledDistribution, SupportedDistribution};
use crate::fixed_point::Fixed;
use std::f64::consts::PI;

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

    pub fn to_float_distribution(self) -> Result<NormalDistribution, String> {
        NormalDistribution::new(self.mu.to_f64(), self.sigma.to_f64())
    }
}

pub fn fixed_calculate_lambda(sigma: Fixed, k: Fixed) -> Result<Fixed, String> {
    let sigma_value = sigma.to_f64();
    let k_value = k.to_f64();
    let lambda = k_value * (2.0 * sigma_value * PI.sqrt()).sqrt();
    Fixed::from_f64(lambda)
}

pub fn fixed_calculate_f(
    x: Fixed,
    distribution: FixedNormalDistribution,
    k: Fixed,
) -> Result<Fixed, String> {
    let float_distribution = distribution.to_float_distribution()?;
    let lambda = fixed_calculate_lambda(distribution.sigma, k)?.to_f64();
    let scaled =
        ScaledDistribution::new(SupportedDistribution::Normal(float_distribution), lambda)?;
    Fixed::from_f64(scaled.value_at(x.to_f64()))
}

pub fn fixed_calculate_minimum_sigma(k: Fixed, b: Fixed) -> Result<Fixed, String> {
    let value = k.to_f64().powi(2) / (b.to_f64().powi(2) * PI.sqrt());
    Fixed::from_f64(value)
}

pub fn fixed_calculate_maximum_k(sigma: Fixed, b: Fixed) -> Result<Fixed, String> {
    let value = b.to_f64() * (sigma.to_f64() * PI.sqrt()).sqrt();
    Fixed::from_f64(value)
}

pub fn fixed_required_collateral(
    from: FixedNormalDistribution,
    to: FixedNormalDistribution,
    k: Fixed,
) -> Result<Fixed, String> {
    let old_f = ScaledDistribution::from_l2_target(
        SupportedDistribution::Normal(from.to_float_distribution()?),
        k.to_f64(),
    )?;
    let new_f = ScaledDistribution::from_l2_target(
        SupportedDistribution::Normal(to.to_float_distribution()?),
        k.to_f64(),
    )?;
    let old_mu = from.mu.to_f64();
    let new_mu = to.mu.to_f64();
    let sigma = from.sigma.to_f64().max(to.sigma.to_f64());
    let span = (old_mu - new_mu).abs();
    let tail = span + 8.0 * sigma;

    let (lower, upper) = if old_mu < new_mu {
        (new_mu, new_mu + tail)
    } else if old_mu > new_mu {
        (new_mu - tail, new_mu)
    } else {
        (new_mu - 8.0 * sigma, new_mu + 8.0 * sigma)
    };

    let best = maximum_absolute_difference(&old_f, &new_f, lower, upper, 20_000);
    Fixed::from_f64(best)
}

fn maximum_absolute_difference(
    old_f: &ScaledDistribution,
    new_f: &ScaledDistribution,
    lower: f64,
    upper: f64,
    samples: usize,
) -> f64 {
    let mut best = 0.0;

    for step in 0..=samples {
        let x = lower + (upper - lower) * step as f64 / samples as f64;
        let value = (new_f.value_at(x) - old_f.value_at(x)).abs();
        if value > best {
            best = value;
        }
    }

    best
}
