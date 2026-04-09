pub mod distributions;
pub mod fixed_point;
pub mod market;
pub mod market_core;
pub mod math_core;
pub mod normal_math;
pub mod numerical;
pub mod scoring;

pub use distributions::{
    CauchyDistribution, Distribution, NormalDistribution, ScaledDistribution, StudentTDistribution,
    SupportedDistribution, UniformDistribution,
};
pub use fixed_point::Fixed;
pub use market::{DistributionMarket, Resolution, TradeRecord};
pub use normal_math::{
    FixedNormalDistribution, fixed_calculate_f, fixed_calculate_lambda, fixed_calculate_maximum_k,
    fixed_calculate_minimum_sigma, fixed_required_collateral,
};
pub use numerical::{MinimumResult, SearchRange, find_global_minimum, verify_minimum_onchain};

#[cfg(test)]
mod tests;
