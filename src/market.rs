use crate::distributions::{Distribution, ScaledDistribution, SupportedDistribution};
use crate::numerical::{MinimumResult, SearchRange, find_global_minimum};
use crate::scoring::trader_payout;
use std::collections::HashMap;
use std::f64::consts::PI;

#[derive(Clone, Debug, PartialEq)]
pub struct TradeRecord {
    pub id: usize,
    pub old_f: ScaledDistribution,
    pub new_f: ScaledDistribution,
    pub collateral: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Resolution {
    pub outcome: f64,
    pub trader_payouts: Vec<(usize, f64)>,
    pub lp_payouts: HashMap<String, f64>,
    pub cash_remaining: f64,
}

#[derive(Clone, Debug)]
pub struct DistributionMarket {
    pub b: f64,
    pub k: f64,
    pub current_f: ScaledDistribution,
    pub total_lp_shares: f64,
    pub lp_shares: HashMap<String, f64>,
    pub cash: f64,
    pub trades: Vec<TradeRecord>,
}

impl DistributionMarket {
    pub fn new(
        b: f64,
        k: f64,
        initial_distribution: SupportedDistribution,
    ) -> Result<Self, String> {
        if !b.is_finite() || !k.is_finite() || b <= 0.0 || k <= 0.0 {
            return Err("b and k must be positive and finite".to_string());
        }

        let current_f = ScaledDistribution::from_l2_target(initial_distribution, k)?;
        validate_solvency(b, &current_f)?;

        let mut lp_shares = HashMap::new();
        lp_shares.insert("genesis_lp".to_string(), 1.0);

        Ok(Self {
            b,
            k,
            current_f,
            total_lp_shares: 1.0,
            lp_shares,
            cash: b,
            trades: Vec::new(),
        })
    }

    pub fn get_market_distribution(&self) -> SupportedDistribution {
        self.current_f.distribution.clone()
    }

    pub fn compute_lambda(&self, distribution: &impl Distribution) -> f64 {
        self.k / distribution.l2_norm()
    }

    pub fn minimum_sigma(&self) -> f64 {
        self.k.powi(2) / (self.b.powi(2) * PI.sqrt())
    }

    pub fn trade(&mut self, new_distribution: SupportedDistribution) -> Result<f64, String> {
        validate_distribution_for_trade(self.b, self.k, &new_distribution)?;
        let new_lambda = self.compute_lambda(&new_distribution);
        let new_f = ScaledDistribution::new(new_distribution, new_lambda)?;
        validate_solvency(self.b, &new_f)?;

        let collateral = self.compute_collateral(&self.current_f, &new_f)?;
        let trade_id = self.trades.len();
        let trade = TradeRecord {
            id: trade_id,
            old_f: self.current_f.clone(),
            new_f: new_f.clone(),
            collateral,
        };

        self.cash += collateral;
        self.current_f = new_f;
        self.trades.push(trade);

        Ok(collateral)
    }

    pub fn compute_collateral(
        &self,
        old_f: &ScaledDistribution,
        new_f: &ScaledDistribution,
    ) -> Result<f64, String> {
        let range = self.default_search_range(old_f, new_f)?;
        let minimum = find_global_minimum(old_f, new_f, range)?;
        Ok((-minimum.value).max(0.0))
    }

    pub fn add_liquidity(
        &mut self,
        lp_id: impl Into<String>,
        proportion: f64,
    ) -> Result<f64, String> {
        if !proportion.is_finite() || proportion <= 0.0 {
            return Err("liquidity proportion must be positive and finite".to_string());
        }

        let lp_id = lp_id.into();
        let backing_added = self.b * proportion;
        let new_shares = self.total_lp_shares * proportion;

        self.b += backing_added;
        self.k += self.k * proportion;
        self.current_f = self.current_f.scale(1.0 + proportion)?;
        self.total_lp_shares += new_shares;
        self.cash += backing_added;
        *self.lp_shares.entry(lp_id).or_insert(0.0) += new_shares;

        Ok(new_shares)
    }

    pub fn remove_liquidity(&mut self, lp_id: &str, shares: f64) -> Result<f64, String> {
        if !shares.is_finite() || shares <= 0.0 {
            return Err("shares must be positive and finite".to_string());
        }

        let current_shares = self
            .lp_shares
            .get(lp_id)
            .copied()
            .ok_or_else(|| "unknown LP id".to_string())?;

        if shares > current_shares + 1e-12 {
            return Err("cannot remove more shares than owned".to_string());
        }

        let proportion = shares / self.total_lp_shares;
        let backing_removed = self.b * proportion;
        let scale_factor = 1.0 - proportion;

        self.b -= backing_removed;
        self.k *= scale_factor;
        self.current_f = self.current_f.scale(scale_factor)?;
        self.total_lp_shares -= shares;
        self.cash -= backing_removed;

        if let Some(balance) = self.lp_shares.get_mut(lp_id) {
            *balance -= shares;
            if balance.abs() < 1e-12 {
                self.lp_shares.remove(lp_id);
            }
        }

        Ok(backing_removed)
    }

    pub fn resolve(&self, outcome: f64) -> Result<Resolution, String> {
        let mut trader_payouts = Vec::with_capacity(self.trades.len());
        let mut total_trader_payout = 0.0;

        for trade in &self.trades {
            let payout = trader_payout(&trade.old_f, &trade.new_f, outcome, trade.collateral);
            if payout < -1e-9 {
                return Err("negative payout encountered during resolution".to_string());
            }

            total_trader_payout += payout.max(0.0);
            trader_payouts.push((trade.id, payout.max(0.0)));
        }

        if total_trader_payout > self.cash + 1e-8 {
            return Err("market is insolvent at resolution".to_string());
        }

        let remaining_for_lps = (self.cash - total_trader_payout).max(0.0);
        let mut lp_payouts = HashMap::new();

        for (lp_id, shares) in &self.lp_shares {
            let payout = if self.total_lp_shares > 0.0 {
                remaining_for_lps * (shares / self.total_lp_shares)
            } else {
                0.0
            };
            lp_payouts.insert(lp_id.clone(), payout);
        }

        Ok(Resolution {
            outcome,
            trader_payouts,
            lp_payouts,
            cash_remaining: 0.0,
        })
    }

    pub fn last_minimum(
        &self,
        old_f: &ScaledDistribution,
        new_f: &ScaledDistribution,
    ) -> Result<MinimumResult, String> {
        let range = self.default_search_range(old_f, new_f)?;
        find_global_minimum(old_f, new_f, range)
    }

    fn default_search_range(
        &self,
        old_f: &ScaledDistribution,
        new_f: &ScaledDistribution,
    ) -> Result<SearchRange, String> {
        if let (Some(old_mu), Some(old_sigma), Some(new_mu), Some(new_sigma)) = (
            old_f.mean_hint(),
            old_f.sigma_hint(),
            new_f.mean_hint(),
            new_f.sigma_hint(),
        ) {
            let sigma_span = old_sigma.max(new_sigma);
            let mean_span = (old_mu - new_mu).abs();
            return SearchRange::new(
                old_mu.min(new_mu) - mean_span - 8.0 * sigma_span,
                old_mu.max(new_mu) + mean_span + 8.0 * sigma_span,
            );
        }

        SearchRange::new(-1_000.0, 1_000.0)
    }
}

fn validate_distribution_for_trade(
    b: f64,
    k: f64,
    distribution: &SupportedDistribution,
) -> Result<(), String> {
    let minimum_scale = k.powi(2) / (b.powi(2) * PI.sqrt());

    match distribution {
        SupportedDistribution::Normal(normal) => {
            if normal.sigma + 1e-12 < minimum_scale {
                return Err(format!(
                    "normal sigma violates solvency bound: sigma={} < sigma_min={}",
                    normal.sigma, minimum_scale
                ));
            }
        }
        SupportedDistribution::Cauchy(cauchy) => {
            if cauchy.gamma + 1e-12 < minimum_scale {
                return Err(format!(
                    "cauchy scale violates solvency bound: gamma={} < gamma_min={}",
                    cauchy.gamma, minimum_scale
                ));
            }
        }
        SupportedDistribution::StudentT(student_t) => {
            if student_t.scale + 1e-12 < minimum_scale {
                return Err(format!(
                    "student t scale violates solvency bound: scale={} < scale_min={}",
                    student_t.scale, minimum_scale
                ));
            }
        }
        SupportedDistribution::Uniform(_) => {}
    }

    Ok(())
}

fn validate_solvency(b: f64, f: &ScaledDistribution) -> Result<(), String> {
    if f.max_value() > b + 1e-9 {
        return Err(format!(
            "distribution exceeds backing at its peak: max(f)={} > b={}",
            f.max_value(),
            b
        ));
    }

    Ok(())
}
