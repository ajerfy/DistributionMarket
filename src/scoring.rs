use crate::distributions::ScaledDistribution;
use crate::numerical::{SearchRange, find_global_minimum, position_difference};

pub fn trader_position_value(
    old_f: &ScaledDistribution,
    new_f: &ScaledDistribution,
    outcome: f64,
) -> f64 {
    position_difference(outcome, old_f, new_f)
}

pub fn trader_payout(
    old_f: &ScaledDistribution,
    new_f: &ScaledDistribution,
    outcome: f64,
    collateral: f64,
) -> f64 {
    collateral + trader_position_value(old_f, new_f, outcome)
}

pub fn trader_profit_and_loss(
    old_f: &ScaledDistribution,
    new_f: &ScaledDistribution,
    outcome: f64,
    collateral: f64,
) -> f64 {
    trader_payout(old_f, new_f, outcome, collateral) - collateral
}

pub fn collateral_is_sufficient(
    old_f: &ScaledDistribution,
    new_f: &ScaledDistribution,
    collateral: f64,
    search_range: SearchRange,
) -> Result<bool, String> {
    let minimum = find_global_minimum(old_f, new_f, search_range)?;
    Ok(collateral + minimum.value >= -1e-9)
}
