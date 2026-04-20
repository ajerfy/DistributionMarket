pub mod distributions;
pub mod fixed_point;
pub mod market;
pub mod market_core;
pub mod math_core;
pub mod normal_market;
pub mod normal_math;
pub mod numerical;
pub mod scoring;
pub mod solana_v1;
pub mod simulation;

pub use distributions::{
    CauchyDistribution, Distribution, NormalDistribution, ScaledDistribution, StudentTDistribution,
    SupportedDistribution, UniformDistribution,
};
pub use fixed_point::Fixed;
pub use market::{DistributionMarket, Resolution, TradeRecord};
pub use normal_market::{
    FixedNormalMarket, FixedNormalResolution, FixedNormalTradeQuote, FixedNormalTradeRecord,
};
pub use normal_math::{
    FixedCollateralQuote, FixedNormalDistribution, FixedSearchBounds, fixed_calculate_f,
    fixed_calculate_lambda, fixed_calculate_maximum_k, fixed_calculate_minimum_sigma,
    fixed_collateral_search_bounds, fixed_required_collateral, fixed_required_collateral_quote,
};
pub use numerical::{MinimumResult, SearchRange, find_global_minimum, verify_minimum_onchain};
pub use solana_v1::{
    LiquidityAction, MarketStatus, NormalPositionSide, OracleConfigV1, QuoteEnvelopeV1,
    ResolveMarketArgsV1, SettleLpArgsV1, SettlePositionArgsV1, SolanaInstructionV1,
    SolanaMarketAccountV1, SolanaNormalPositionAccountV1, SolanaOperationMappingV1,
    SolanaPositionSettlementV1, TradeArgsV1, current_normal_market_to_account,
    normal_v1_operation_mapping,
};
pub use simulation::{
    SimulationReport, SimulationScenario, SimulationStep, builtin_scenarios, find_scenario,
    render_report, run_scenario,
};

#[cfg(test)]
mod tests;
