use statrs::function::erf::erf;
use statrs::function::gamma::gamma;
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Support {
    pub lower: f64,
    pub upper: f64,
}

impl Support {
    pub fn new(lower: f64, upper: f64) -> Result<Self, String> {
        if !lower.is_finite() || !upper.is_finite() || upper <= lower {
            return Err("support must be finite with upper > lower".to_string());
        }

        Ok(Self { lower, upper })
    }
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
pub struct CauchyDistribution {
    pub x0: f64,
    pub gamma: f64,
}

impl CauchyDistribution {
    pub fn new(x0: f64, gamma: f64) -> Result<Self, String> {
        if !gamma.is_finite() || gamma <= 0.0 {
            return Err("cauchy scale must be positive and finite".to_string());
        }

        Ok(Self { x0, gamma })
    }
}

impl Distribution for CauchyDistribution {
    fn pdf(&self, x: f64) -> f64 {
        let z = (x - self.x0) / self.gamma;
        1.0 / (PI * self.gamma * (1.0 + z * z))
    }

    fn l2_norm(&self) -> f64 {
        (1.0 / (2.0 * PI * self.gamma)).sqrt()
    }

    fn max_pdf(&self) -> f64 {
        1.0 / (PI * self.gamma)
    }

    fn cdf(&self, x: f64) -> f64 {
        0.5 + ((x - self.x0) / self.gamma).atan() / PI
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StudentTDistribution {
    pub mu: f64,
    pub scale: f64,
    pub nu: f64,
}

impl StudentTDistribution {
    pub fn new(mu: f64, scale: f64, nu: f64) -> Result<Self, String> {
        if !scale.is_finite() || scale <= 0.0 {
            return Err("student t scale must be positive and finite".to_string());
        }

        if !nu.is_finite() || nu <= 0.0 {
            return Err("student t degrees of freedom must be positive and finite".to_string());
        }

        Ok(Self { mu, scale, nu })
    }

    fn normalization_constant(&self) -> f64 {
        gamma((self.nu + 1.0) / 2.0) / (gamma(self.nu / 2.0) * (self.nu * PI).sqrt())
    }
}

impl Distribution for StudentTDistribution {
    fn pdf(&self, x: f64) -> f64 {
        let z = (x - self.mu) / self.scale;
        let exponent = -(self.nu + 1.0) / 2.0;
        self.normalization_constant() * (1.0 + z * z / self.nu).powf(exponent) / self.scale
    }

    fn l2_norm(&self) -> f64 {
        let numerator = gamma((self.nu + 1.0) / 2.0).powi(2) * gamma(self.nu + 0.5);
        let denominator = gamma(self.nu / 2.0).powi(2)
            * gamma(self.nu + 1.0)
            * self.scale
            * (self.nu * PI).sqrt();
        (numerator / denominator).sqrt()
    }

    fn max_pdf(&self) -> f64 {
        self.pdf(self.mu)
    }

    fn cdf(&self, x: f64) -> f64 {
        let distribution = statrs::distribution::StudentsT::new(self.mu, self.scale, self.nu)
            .expect("validated student t parameters should construct");
        use statrs::distribution::ContinuousCDF;
        distribution.cdf(x)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SupportedDistribution {
    Normal(NormalDistribution),
    Uniform(UniformDistribution),
    Cauchy(CauchyDistribution),
    StudentT(StudentTDistribution),
}

impl SupportedDistribution {
    pub fn normal(mu: f64, sigma: f64) -> Result<Self, String> {
        Ok(Self::Normal(NormalDistribution::new(mu, sigma)?))
    }

    pub fn uniform(a: f64, b: f64) -> Result<Self, String> {
        Ok(Self::Uniform(UniformDistribution::new(a, b)?))
    }

    pub fn cauchy(x0: f64, gamma: f64) -> Result<Self, String> {
        Ok(Self::Cauchy(CauchyDistribution::new(x0, gamma)?))
    }

    pub fn student_t(mu: f64, scale: f64, nu: f64) -> Result<Self, String> {
        Ok(Self::StudentT(StudentTDistribution::new(mu, scale, nu)?))
    }

    pub fn location_hint(&self) -> f64 {
        match self {
            Self::Normal(distribution) => distribution.mu,
            Self::Uniform(distribution) => (distribution.a + distribution.b) / 2.0,
            Self::Cauchy(distribution) => distribution.x0,
            Self::StudentT(distribution) => distribution.mu,
        }
    }

    pub fn scale_hint(&self) -> f64 {
        match self {
            Self::Normal(distribution) => distribution.sigma,
            Self::Uniform(distribution) => (distribution.b - distribution.a) / 2.0,
            Self::Cauchy(distribution) => distribution.gamma,
            Self::StudentT(distribution) => distribution.scale,
        }
    }

    pub fn finite_support(&self) -> Option<Support> {
        match self {
            Self::Uniform(distribution) => Support::new(distribution.a, distribution.b).ok(),
            Self::Normal(_) | Self::Cauchy(_) | Self::StudentT(_) => None,
        }
    }

    pub fn search_tail_factor(&self) -> f64 {
        match self {
            Self::Normal(_) => 8.0,
            Self::Uniform(_) => 1.0,
            Self::Cauchy(_) => 32.0,
            Self::StudentT(distribution) => {
                if distribution.nu <= 2.0 {
                    28.0
                } else if distribution.nu <= 5.0 {
                    20.0
                } else {
                    12.0
                }
            }
        }
    }
}

impl Distribution for SupportedDistribution {
    fn pdf(&self, x: f64) -> f64 {
        match self {
            Self::Normal(distribution) => distribution.pdf(x),
            Self::Uniform(distribution) => distribution.pdf(x),
            Self::Cauchy(distribution) => distribution.pdf(x),
            Self::StudentT(distribution) => distribution.pdf(x),
        }
    }

    fn l2_norm(&self) -> f64 {
        match self {
            Self::Normal(distribution) => distribution.l2_norm(),
            Self::Uniform(distribution) => distribution.l2_norm(),
            Self::Cauchy(distribution) => distribution.l2_norm(),
            Self::StudentT(distribution) => distribution.l2_norm(),
        }
    }

    fn max_pdf(&self) -> f64 {
        match self {
            Self::Normal(distribution) => distribution.max_pdf(),
            Self::Uniform(distribution) => distribution.max_pdf(),
            Self::Cauchy(distribution) => distribution.max_pdf(),
            Self::StudentT(distribution) => distribution.max_pdf(),
        }
    }

    fn cdf(&self, x: f64) -> f64 {
        match self {
            Self::Normal(distribution) => distribution.cdf(x),
            Self::Uniform(distribution) => distribution.cdf(x),
            Self::Cauchy(distribution) => distribution.cdf(x),
            Self::StudentT(distribution) => distribution.cdf(x),
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
        Some(self.distribution.location_hint())
    }

    pub fn sigma_hint(&self) -> Option<f64> {
        Some(self.distribution.scale_hint())
    }

    pub fn support_hint(&self) -> Option<Support> {
        self.distribution.finite_support()
    }

    pub fn search_tail_factor(&self) -> f64 {
        self.distribution.search_tail_factor()
    }
}
