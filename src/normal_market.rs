use crate::fixed_point::Fixed;
use crate::normal_math::{
    FixedNormalDistribution, fixed_calculate_f, fixed_calculate_lambda,
    fixed_calculate_minimum_sigma, fixed_calculate_value_from_lambda, fixed_required_collateral,
};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedNormalTradeRecord {
    pub id: usize,
    pub old_distribution: FixedNormalDistribution,
    pub new_distribution: FixedNormalDistribution,
    pub collateral: Fixed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedNormalResolution {
    pub outcome: Fixed,
    pub trader_payouts: Vec<(usize, Fixed)>,
    pub lp_payouts: HashMap<String, Fixed>,
    pub cash_remaining: Fixed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedNormalMarket {
    pub b: Fixed,
    pub k: Fixed,
    pub current_distribution: FixedNormalDistribution,
    pub current_lambda: Fixed,
    pub total_lp_shares: Fixed,
    pub lp_shares: HashMap<String, Fixed>,
    pub cash: Fixed,
    pub trades: Vec<FixedNormalTradeRecord>,
}

impl FixedNormalMarket {
    pub fn new(
        b: Fixed,
        k: Fixed,
        initial_distribution: FixedNormalDistribution,
    ) -> Result<Self, String> {
        if b.raw() <= 0 || k.raw() <= 0 {
            return Err("b and k must be positive".to_string());
        }

        let current_lambda = fixed_calculate_lambda(initial_distribution.sigma, k)?;
        validate_fixed_solvency(b, current_lambda, initial_distribution)?;

        let mut lp_shares = HashMap::new();
        lp_shares.insert("genesis_lp".to_string(), Fixed::ONE);

        Ok(Self {
            b,
            k,
            current_distribution: initial_distribution,
            current_lambda,
            total_lp_shares: Fixed::ONE,
            lp_shares,
            cash: b,
            trades: Vec::new(),
        })
    }

    pub fn minimum_sigma(&self) -> Result<Fixed, String> {
        fixed_calculate_minimum_sigma(self.k, self.b)
    }

    pub fn trade(&mut self, new_distribution: FixedNormalDistribution) -> Result<Fixed, String> {
        let min_sigma = self.minimum_sigma()?;
        if new_distribution.sigma.raw() < min_sigma.raw() {
            return Err(format!(
                "normal sigma violates solvency bound: sigma={} < sigma_min={}",
                new_distribution.sigma, min_sigma
            ));
        }

        let new_lambda = fixed_calculate_lambda(new_distribution.sigma, self.k)?;
        validate_fixed_solvency(self.b, new_lambda, new_distribution)?;

        let collateral =
            fixed_required_collateral(self.current_distribution, new_distribution, self.k)?;
        let trade_id = self.trades.len();
        self.cash = self.cash + collateral;
        self.trades.push(FixedNormalTradeRecord {
            id: trade_id,
            old_distribution: self.current_distribution,
            new_distribution,
            collateral,
        });
        self.current_distribution = new_distribution;
        self.current_lambda = new_lambda;

        Ok(collateral)
    }

    pub fn add_liquidity(
        &mut self,
        lp_id: impl Into<String>,
        proportion: Fixed,
    ) -> Result<Fixed, String> {
        if proportion.raw() <= 0 {
            return Err("liquidity proportion must be positive".to_string());
        }

        let lp_id = lp_id.into();
        let backing_added = self.b * proportion;
        let new_shares = self.total_lp_shares * proportion;
        self.b = self.b + backing_added;
        self.k = self.k + (self.k * proportion);
        self.current_lambda = self.current_lambda + (self.current_lambda * proportion);
        self.total_lp_shares = self.total_lp_shares + new_shares;
        self.cash = self.cash + backing_added;
        *self.lp_shares.entry(lp_id).or_insert(Fixed::ZERO) =
            self.lp_shares.get(&lp_id).copied().unwrap_or(Fixed::ZERO) + new_shares;
        Ok(new_shares)
    }

    pub fn resolve(&self, outcome: Fixed) -> Result<FixedNormalResolution, String> {
        let mut trader_payouts = Vec::with_capacity(self.trades.len());
        let mut total_trader_payout = Fixed::ZERO;

        for trade in &self.trades {
            let final_payout = fixed_calculate_f(outcome, trade.new_distribution, self.k)?;
            let initial_payout = fixed_calculate_f(outcome, trade.old_distribution, self.k)?;
            let signed_delta = final_payout - initial_payout;
            let payout = trade.collateral + signed_delta;

            if payout.raw() < 0 {
                return Err("negative trader payout encountered during resolution".to_string());
            }

            total_trader_payout = total_trader_payout + payout;
            trader_payouts.push((trade.id, payout));
        }

        if total_trader_payout.raw() > self.cash.raw() {
            return Err("market is insolvent at resolution".to_string());
        }

        let remaining_for_lps = self.cash - total_trader_payout;
        let mut lp_payouts = HashMap::new();
        for (lp_id, shares) in &self.lp_shares {
            let payout = if self.total_lp_shares.raw() > 0 {
                remaining_for_lps * (*shares / self.total_lp_shares)
            } else {
                Fixed::ZERO
            };
            lp_payouts.insert(lp_id.clone(), payout);
        }

        Ok(FixedNormalResolution {
            outcome,
            trader_payouts,
            lp_payouts,
            cash_remaining: Fixed::ZERO,
        })
    }
}

fn validate_fixed_solvency(
    b: Fixed,
    lambda: Fixed,
    distribution: FixedNormalDistribution,
) -> Result<(), String> {
    let peak = fixed_calculate_value_from_lambda(distribution.mu, distribution, lambda)?;
    if peak.raw() > b.raw() {
        return Err(format!(
            "distribution exceeds backing at its peak: max(f)={} > b={}",
            peak, b
        ));
    }
    Ok(())
}
