use distribution_markets::{DistributionMarket, SupportedDistribution};

fn main() -> Result<(), String> {
    run_example(
        "Student's t Distribution Market Simulation",
        SupportedDistribution::student_t(0.0, 1.25, 5.0)?,
        SupportedDistribution::student_t(1.0, 1.25, 5.0)?,
        1.2,
    )
}

fn run_example(
    title: &str,
    initial_distribution: SupportedDistribution,
    trader_distribution: SupportedDistribution,
    outcome: f64,
) -> Result<(), String> {
    let mut market = DistributionMarket::new(10.0, 3.0, initial_distribution.clone())?;

    println!("{title}");
    println!("{}", "=".repeat(title.len()));
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

    let collateral = market.trade(trader_distribution.clone())?;
    println!("Trader move");
    println!(
        "  new distribution: {}",
        describe_distribution(&trader_distribution)
    );
    println!("  required collateral: {:.6}", collateral);
    println!("  market cash after trade: {:.6}", market.cash);
    println!();

    let resolution = market.resolve(outcome)?;
    println!("Resolution");
    println!("  realized outcome: {:.4}", resolution.outcome);
    for (trade_id, payout) in &resolution.trader_payouts {
        println!("  trade #{trade_id} payout: {:.6}", payout);
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
