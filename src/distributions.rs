use statrs::function::erf::erf;
use std::f64::consts::{PI, SQRT_2};

/// A probability distribution that can be scaled into an outcome-token position.
pub trait Distribution {
    /// Returns the probability density at `x`.
    fn pdf(&self, x: f64) -> f64;

    /// Returns the L2 norm of the probability density.
    fn l2_norm(&self) -> f64;

    /// Returns the peak density, which determines the solvency constraint `max(f) <= b`.
    fn max_pdf(&self) -> f64;

    /// Returns the cumulative distribution value at `x`.
    fn cdf(&self, x: f64) -> f64;
}

#[derive(Clone, Debug, PartialEq)]
pub struct NormalDistribution {
    pub mu: f64,
    pub sigma: f64,
}

impl NormalDistribution {
    pub fn new(mu: f64, sigma: f64) -> Result<Self, String> {
        if !sigma.is_finite() || sigma <= 0.0 {
            return Err("normal sigma must be positive and finite".to_string());
        }

        Ok(Self { mu, sigma })
    }
}

impl Distribution for NormalDistribution {
    fn pdf(&self, x: f64) -> f64 {
        let standardized = (x - self.mu) / self.sigma;
        let exponent = -0.5 * standardized * standardized;
        exponent.exp() / (self.sigma * (2.0 * PI).sqrt())
    }

    fn l2_norm(&self) -> f64 {
        (1.0 / (2.0 * self.sigma * PI.sqrt())).sqrt()
    }

    fn max_pdf(&self) -> f64 {
        1.0 / (self.sigma * (2.0 * PI).sqrt())
    }

    fn cdf(&self, x: f64) -> f64 {
        let z = (x - self.mu) / (self.sigma * SQRT_2);
        0.5 * (1.0 + erf(z))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UniformDistribution {
    pub a: f64,
    pub b: f64,
}

impl UniformDistribution {
    pub fn new(a: f64, b: f64) -> Result<Self, String> {
        if !a.is_finite() || !b.is_finite() || b <= a {
            return Err("uniform bounds must be finite and satisfy b > a".to_string());
        }

        Ok(Self { a, b })
    }

    fn width(&self) -> f64 {
        self.b - self.a
    }
}

impl Distribution for UniformDistribution {
    fn pdf(&self, x: f64) -> f64 {
        if x < self.a || x > self.b {
            0.0
        } else {
            1.0 / self.width()
        }
    }

    fn l2_norm(&self) -> f64 {
        (1.0 / self.width()).sqrt()
    }

    fn max_pdf(&self) -> f64 {
        1.0 / self.width()
    }

    fn cdf(&self, x: f64) -> f64 {
        if x <= self.a {
            0.0
        } else if x >= self.b {
            1.0
        } else {
            (x - self.a) / self.width()
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SupportedDistribution {
    Normal(NormalDistribution),
    Uniform(UniformDistribution),
}

impl SupportedDistribution {
    pub fn normal(mu: f64, sigma: f64) -> Result<Self, String> {
        Ok(Self::Normal(NormalDistribution::new(mu, sigma)?))
    }

    pub fn uniform(a: f64, b: f64) -> Result<Self, String> {
        Ok(Self::Uniform(UniformDistribution::new(a, b)?))
    }

    pub fn as_normal(&self) -> Option<&NormalDistribution> {
        match self {
            Self::Normal(distribution) => Some(distribution),
            Self::Uniform(_) => None,
        }
    }
}

impl Distribution for SupportedDistribution {
    fn pdf(&self, x: f64) -> f64 {
        match self {
            Self::Normal(distribution) => distribution.pdf(x),
            Self::Uniform(distribution) => distribution.pdf(x),
        }
    }

    fn l2_norm(&self) -> f64 {
        match self {
            Self::Normal(distribution) => distribution.l2_norm(),
            Self::Uniform(distribution) => distribution.l2_norm(),
        }
    }

    fn max_pdf(&self) -> f64 {
        match self {
            Self::Normal(distribution) => distribution.max_pdf(),
            Self::Uniform(distribution) => distribution.max_pdf(),
        }
    }

    fn cdf(&self, x: f64) -> f64 {
        match self {
            Self::Normal(distribution) => distribution.cdf(x),
            Self::Uniform(distribution) => distribution.cdf(x),
        }
    }
}

/// A trader or AMM position `f = λ p`, where `p` is a normalized probability density.
#[derive(Clone, Debug, PartialEq)]
pub struct ScaledDistribution {
    pub distribution: SupportedDistribution,
    pub lambda: f64,
}

impl ScaledDistribution {
    pub fn new(distribution: SupportedDistribution, lambda: f64) -> Result<Self, String> {
        if !lambda.is_finite() || lambda < 0.0 {
            return Err("lambda must be non-negative and finite".to_string());
        }

        Ok(Self {
            distribution,
            lambda,
        })
    }

    pub fn from_l2_target(distribution: SupportedDistribution, k: f64) -> Result<Self, String> {
        if !k.is_finite() || k <= 0.0 {
            return Err("k must be positive and finite".to_string());
        }

        let lambda = k / distribution.l2_norm();
        Self::new(distribution, lambda)
    }

    pub fn value_at(&self, x: f64) -> f64 {
        self.lambda * self.distribution.pdf(x)
    }

    pub fn l2_norm(&self) -> f64 {
        self.lambda * self.distribution.l2_norm()
    }

    pub fn max_value(&self) -> f64 {
        self.lambda * self.distribution.max_pdf()
    }

    pub fn scale(&self, factor: f64) -> Result<Self, String> {
        if !factor.is_finite() || factor < 0.0 {
            return Err("scale factor must be non-negative and finite".to_string());
        }

        Self::new(self.distribution.clone(), self.lambda * factor)
    }

    pub fn mean_hint(&self) -> Option<f64> {
        self.distribution
            .as_normal()
            .map(|distribution| distribution.mu)
    }

    pub fn sigma_hint(&self) -> Option<f64> {
        self.distribution
            .as_normal()
            .map(|distribution| distribution.sigma)
    }
}
