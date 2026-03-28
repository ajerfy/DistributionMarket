use distribution_markets::{DistributionMarket, SupportedDistribution};

fn main() -> Result<(), String> {
    let initial_distribution = SupportedDistribution::normal(95.0, 10.0)?;
    let mut market =
        DistributionMarket::new(50.0, 21.05026039569057, initial_distribution.clone())?;

    println!("Distribution Market Simulation");
    println!("==============================");
    println!();
    println!("Initial market");
    println!("  backing (b): {:.4}", market.b);
    println!("  invariant (k): {:.6}", market.k);
    println!(
        "  distribution: {}",
        describe_distribution(&initial_distribution)
    );
    println!("  lambda: {:.6}", market.current_f.lambda);
    println!("  max f(x): {:.6}", market.current_f.max_value());
    println!();

    let trader_distribution = SupportedDistribution::normal(100.0, 10.0)?;
    let collateral = market.trade(trader_distribution.clone())?;
    println!("Trader move");
    println!(
        "  new distribution: {}",
        describe_distribution(&trader_distribution)
    );
    println!("  required collateral: {:.6}", collateral);
    println!("  market cash after trade: {:.6}", market.cash);
    println!();

    let minted_shares = market.add_liquidity("lp_2", 0.5)?;
    println!("Liquidity addition");
    println!("  LP id: lp_2");
    println!("  proportion added: 50.00%");
    println!("  minted LP shares: {:.6}", minted_shares);
    println!("  new backing (b): {:.6}", market.b);
    println!("  new invariant (k): {:.6}", market.k);
    println!("  scaled lambda: {:.6}", market.current_f.lambda);
    println!();

    let outcome = 107.6;
    let resolution = market.resolve(outcome)?;
    println!("Resolution");
    println!("  realized outcome: {:.4}", resolution.outcome);
    println!("  trader payouts:");
    for (trade_id, payout) in &resolution.trader_payouts {
        println!("    trade #{trade_id}: {:.6}", payout);
    }
    println!("  LP payouts:");
    let mut lp_entries: Vec<_> = resolution.lp_payouts.iter().collect();
    lp_entries.sort_by(|left, right| left.0.cmp(right.0));
    for (lp_id, payout) in lp_entries {
        println!("    {lp_id}: {:.6}", payout);
    }

    Ok(())
}

fn describe_distribution(distribution: &SupportedDistribution) -> String {
    match distribution {
        SupportedDistribution::Normal(normal) => {
            format!("Normal(mu={:.4}, sigma={:.4})", normal.mu, normal.sigma)
        }
        SupportedDistribution::Uniform(uniform) => {
            format!("Uniform(a={:.4}, b={:.4})", uniform.a, uniform.b)
        }
    }
}
