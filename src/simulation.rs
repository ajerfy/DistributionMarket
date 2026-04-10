use crate::distributions::Distribution;
use crate::{
    DistributionMarket, Fixed, FixedNormalDistribution, FixedNormalMarket, SupportedDistribution,
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum SimulationStep {
    Trade { distribution: SupportedDistribution },
    AddLiquidity { lp_id: String, proportion: f64 },
}

#[derive(Clone, Debug)]
pub struct SimulationScenario {
    pub slug: &'static str,
    pub title: &'static str,
    pub initial_backing: f64,
    pub initial_k: f64,
    pub initial_distribution: SupportedDistribution,
    pub steps: Vec<SimulationStep>,
    pub outcome: f64,
}

#[derive(Clone, Debug)]
pub struct TradeSummary {
    pub distribution_label: String,
    pub collateral: f64,
    pub market_cash_after_trade: f64,
}

#[derive(Clone, Debug)]
pub struct LiquiditySummary {
    pub lp_id: String,
    pub proportion: f64,
    pub minted_shares: f64,
    pub new_backing: f64,
    pub new_k: f64,
    pub scaled_lambda: f64,
}

#[derive(Clone, Debug)]
pub struct SimulationReport {
    pub title: String,
    pub scenario_slug: String,
    pub initial_backing: f64,
    pub initial_k: f64,
    pub initial_distribution_label: String,
    pub initial_lambda: f64,
    pub initial_max_value: f64,
    pub trades: Vec<TradeSummary>,
    pub liquidity_events: Vec<LiquiditySummary>,
    pub outcome: f64,
    pub trader_payouts: Vec<(usize, f64)>,
    pub lp_payouts: HashMap<String, f64>,
}

pub fn builtin_scenarios() -> Vec<SimulationScenario> {
    vec![
        SimulationScenario {
            slug: "normal",
            title: "Distribution Market Simulation",
            initial_backing: 50.0,
            initial_k: 21.05026039569057,
            initial_distribution: SupportedDistribution::normal(95.0, 10.0)
                .expect("valid normal scenario"),
            steps: vec![
                SimulationStep::Trade {
                    distribution: SupportedDistribution::normal(100.0, 10.0)
                        .expect("valid normal scenario"),
                },
                SimulationStep::AddLiquidity {
                    lp_id: "lp_2".to_string(),
                    proportion: 0.5,
                },
            ],
            outcome: 107.6,
        },
        SimulationScenario {
            slug: "uniform",
            title: "Uniform Distribution Market Simulation",
            initial_backing: 10.0,
            initial_k: 3.0,
            initial_distribution: SupportedDistribution::uniform(-2.0, 2.0)
                .expect("valid uniform scenario"),
            steps: vec![SimulationStep::Trade {
                distribution: SupportedDistribution::uniform(0.0, 4.0)
                    .expect("valid uniform scenario"),
            }],
            outcome: 1.5,
        },
        SimulationScenario {
            slug: "cauchy",
            title: "Cauchy Distribution Market Simulation",
            initial_backing: 10.0,
            initial_k: 3.0,
            initial_distribution: SupportedDistribution::cauchy(0.0, 2.0)
                .expect("valid cauchy scenario"),
            steps: vec![SimulationStep::Trade {
                distribution: SupportedDistribution::cauchy(1.5, 2.0)
                    .expect("valid cauchy scenario"),
            }],
            outcome: 1.75,
        },
        SimulationScenario {
            slug: "student_t",
            title: "Student's t Distribution Market Simulation",
            initial_backing: 10.0,
            initial_k: 3.0,
            initial_distribution: SupportedDistribution::student_t(0.0, 1.25, 5.0)
                .expect("valid student t scenario"),
            steps: vec![SimulationStep::Trade {
                distribution: SupportedDistribution::student_t(1.0, 1.25, 5.0)
                    .expect("valid student t scenario"),
            }],
            outcome: 1.2,
        },
    ]
}

pub fn find_scenario(slug: &str) -> Option<SimulationScenario> {
    builtin_scenarios()
        .into_iter()
        .find(|scenario| scenario.slug == slug)
}

pub fn run_scenario(scenario: &SimulationScenario) -> Result<SimulationReport, String> {
    if scenario.slug == "normal" {
        return run_normal_fixed_scenario(scenario);
    }

    let initial_distribution_label = describe_distribution(&scenario.initial_distribution);
    let initial_lambda = scenario.initial_k / scenario.initial_distribution.l2_norm();
    let initial_max_value = initial_lambda * scenario.initial_distribution.max_pdf();
    let mut market = DistributionMarket::new(
        scenario.initial_backing,
        scenario.initial_k,
        scenario.initial_distribution.clone(),
    )?;
    let mut trades = Vec::new();
    let mut liquidity_events = Vec::new();

    for step in &scenario.steps {
        match step {
            SimulationStep::Trade { distribution } => {
                let collateral = market.trade(distribution.clone())?;
                trades.push(TradeSummary {
                    distribution_label: describe_distribution(distribution),
                    collateral,
                    market_cash_after_trade: market.cash,
                });
            }
            SimulationStep::AddLiquidity { lp_id, proportion } => {
                let minted_shares = market.add_liquidity(lp_id.clone(), *proportion)?;
                liquidity_events.push(LiquiditySummary {
                    lp_id: lp_id.clone(),
                    proportion: *proportion,
                    minted_shares,
                    new_backing: market.b,
                    new_k: market.k,
                    scaled_lambda: market.current_f.lambda,
                });
            }
        }
    }

    let resolution = market.resolve(scenario.outcome)?;

    Ok(SimulationReport {
        title: scenario.title.to_string(),
        scenario_slug: scenario.slug.to_string(),
        initial_backing: scenario.initial_backing,
        initial_k: scenario.initial_k,
        initial_distribution_label,
        initial_lambda,
        initial_max_value,
        trades,
        liquidity_events,
        outcome: resolution.outcome,
        trader_payouts: resolution.trader_payouts,
        lp_payouts: resolution.lp_payouts,
    })
}

fn run_normal_fixed_scenario(scenario: &SimulationScenario) -> Result<SimulationReport, String> {
    let initial_distribution = as_fixed_normal_distribution(&scenario.initial_distribution)?;
    let initial_distribution_label = describe_distribution(&scenario.initial_distribution);
    let initial_lambda = scenario.initial_k / scenario.initial_distribution.l2_norm();
    let initial_max_value = initial_lambda * scenario.initial_distribution.max_pdf();
    let mut market = FixedNormalMarket::new(
        Fixed::from_f64(scenario.initial_backing)?,
        Fixed::from_f64(scenario.initial_k)?,
        initial_distribution,
    )?;
    let mut trades = Vec::new();
    let mut liquidity_events = Vec::new();

    for step in &scenario.steps {
        match step {
            SimulationStep::Trade { distribution } => {
                let fixed_distribution = as_fixed_normal_distribution(distribution)?;
                let collateral = market.trade(fixed_distribution)?;
                trades.push(TradeSummary {
                    distribution_label: describe_distribution(distribution),
                    collateral: collateral.to_f64(),
                    market_cash_after_trade: market.cash.to_f64(),
                });
            }
            SimulationStep::AddLiquidity { lp_id, proportion } => {
                let minted_shares =
                    market.add_liquidity(lp_id.clone(), Fixed::from_f64(*proportion)?)?;
                liquidity_events.push(LiquiditySummary {
                    lp_id: lp_id.clone(),
                    proportion: *proportion,
                    minted_shares: minted_shares.to_f64(),
                    new_backing: market.b.to_f64(),
                    new_k: market.k.to_f64(),
                    scaled_lambda: market.current_lambda.to_f64(),
                });
            }
        }
    }

    let resolution = market.resolve(Fixed::from_f64(scenario.outcome)?)?;

    Ok(SimulationReport {
        title: scenario.title.to_string(),
        scenario_slug: scenario.slug.to_string(),
        initial_backing: scenario.initial_backing,
        initial_k: scenario.initial_k,
        initial_distribution_label,
        initial_lambda,
        initial_max_value,
        trades,
        liquidity_events,
        outcome: resolution.outcome.to_f64(),
        trader_payouts: resolution
            .trader_payouts
            .into_iter()
            .map(|(trade_id, payout)| (trade_id, payout.to_f64()))
            .collect(),
        lp_payouts: resolution
            .lp_payouts
            .into_iter()
            .map(|(lp_id, payout)| (lp_id, payout.to_f64()))
            .collect(),
    })
}

pub fn render_report(report: &SimulationReport) -> String {
    let mut output = String::new();
    output.push_str(&format!("{}\n", report.title));
    output.push_str(&format!("{}\n\n", "=".repeat(report.title.len())));
    output.push_str(&format!("Scenario: {}\n\n", report.scenario_slug));
    output.push_str("Initial market\n");
    output.push_str(&format!("  backing (b): {:.4}\n", report.initial_backing));
    output.push_str(&format!("  invariant (k): {:.6}\n", report.initial_k));
    output.push_str(&format!(
        "  distribution: {}\n",
        report.initial_distribution_label
    ));
    output.push_str(&format!("  lambda: {:.6}\n", report.initial_lambda));
    output.push_str(&format!("  max f(x): {:.6}\n\n", report.initial_max_value));

    for trade in &report.trades {
        output.push_str("Trader move\n");
        output.push_str(&format!(
            "  new distribution: {}\n",
            trade.distribution_label
        ));
        output.push_str(&format!("  required collateral: {:.6}\n", trade.collateral));
        output.push_str(&format!(
            "  market cash after trade: {:.6}\n\n",
            trade.market_cash_after_trade
        ));
    }

    for event in &report.liquidity_events {
        output.push_str("Liquidity addition\n");
        output.push_str(&format!("  LP id: {}\n", event.lp_id));
        output.push_str(&format!(
            "  proportion added: {:.2}%\n",
            event.proportion * 100.0
        ));
        output.push_str(&format!("  minted LP shares: {:.6}\n", event.minted_shares));
        output.push_str(&format!("  new backing (b): {:.6}\n", event.new_backing));
        output.push_str(&format!("  new invariant (k): {:.6}\n", event.new_k));
        output.push_str(&format!("  scaled lambda: {:.6}\n\n", event.scaled_lambda));
    }

    output.push_str("Resolution\n");
    output.push_str(&format!("  realized outcome: {:.4}\n", report.outcome));

    if report.trader_payouts.is_empty() {
        output.push_str("  trader payouts: none\n");
    } else {
        output.push_str("  trader payouts:\n");
        for (trade_id, payout) in &report.trader_payouts {
            output.push_str(&format!("    trade #{}: {:.6}\n", trade_id, payout));
        }
    }

    if report.lp_payouts.is_empty() {
        output.push_str("  LP payouts: none\n");
    } else {
        output.push_str("  LP payouts:\n");
        let mut entries: Vec<_> = report.lp_payouts.iter().collect();
        entries.sort_by(|left, right| left.0.cmp(right.0));
        for (lp_id, payout) in entries {
            output.push_str(&format!("    {}: {:.6}\n", lp_id, payout));
        }
    }

    output
}

pub fn describe_distribution(distribution: &SupportedDistribution) -> String {
    match distribution {
        SupportedDistribution::Normal(normal) => {
            format!("Normal(mu={:.4}, sigma={:.4})", normal.mu, normal.sigma)
        }
        SupportedDistribution::Uniform(uniform) => {
            format!("Uniform(a={:.4}, b={:.4})", uniform.a, uniform.b)
        }
        SupportedDistribution::Cauchy(cauchy) => {
            format!("Cauchy(x0={:.4}, gamma={:.4})", cauchy.x0, cauchy.gamma)
        }
        SupportedDistribution::StudentT(student_t) => {
            format!(
                "StudentT(mu={:.4}, scale={:.4}, nu={:.4})",
                student_t.mu, student_t.scale, student_t.nu
            )
        }
    }
}

fn as_fixed_normal_distribution(
    distribution: &SupportedDistribution,
) -> Result<FixedNormalDistribution, String> {
    match distribution {
        SupportedDistribution::Normal(normal) => FixedNormalDistribution::new(
            Fixed::from_f64(normal.mu)?,
            Fixed::from_f64(normal.sigma)?,
        ),
        _ => Err("fixed normal simulation path only supports normal distributions".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{builtin_scenarios, find_scenario, render_report, run_scenario};

    #[test]
    fn built_in_scenarios_are_available() {
        let scenarios = builtin_scenarios();
        assert_eq!(scenarios.len(), 4);
        assert!(scenarios.iter().any(|scenario| scenario.slug == "normal"));
        assert!(scenarios.iter().any(|scenario| scenario.slug == "uniform"));
        assert!(scenarios.iter().any(|scenario| scenario.slug == "cauchy"));
        assert!(
            scenarios
                .iter()
                .any(|scenario| scenario.slug == "student_t")
        );
    }

    #[test]
    fn normal_scenario_runs_and_renders() {
        let scenario = find_scenario("normal").expect("normal scenario exists");
        let report = run_scenario(&scenario).expect("scenario should run");
        let rendered = render_report(&report);
        assert!(rendered.contains("Distribution Market Simulation"));
        assert!(rendered.contains("Liquidity addition"));
        assert!(rendered.contains("Resolution"));
    }
}
