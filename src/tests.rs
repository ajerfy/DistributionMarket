use crate::distributions::{
    CauchyDistribution, Distribution, NormalDistribution, ScaledDistribution, StudentTDistribution,
    SupportedDistribution, UniformDistribution,
};
use crate::market::DistributionMarket;
use crate::numerical::{SearchRange, find_global_minimum, verify_minimum_onchain};
use crate::scoring::{collateral_is_sufficient, trader_payout};

fn assert_close(left: f64, right: f64, tolerance: f64) {
    let delta = (left - right).abs();
    assert!(
        delta <= tolerance,
        "left={left}, right={right}, delta={delta}, tolerance={tolerance}"
    );
}

#[test]
fn normal_distribution_l2_norm_matches_closed_form() {
    let distribution = NormalDistribution::new(0.0, 2.0).unwrap();
    let expected = (1.0 / (4.0 * std::f64::consts::PI.sqrt())).sqrt();
    assert_close(distribution.l2_norm(), expected, 1e-12);
}

#[test]
fn market_distribution_round_trips_from_reserves() {
    let initial = SupportedDistribution::normal(95.0, 10.0).unwrap();
    let market = DistributionMarket::new(50.0, 21.05026039569057, initial.clone()).unwrap();
    assert_eq!(market.get_market_distribution(), initial);
}

#[test]
fn minimum_sigma_constraint_is_enforced() {
    let initial = SupportedDistribution::normal(95.0, 10.0).unwrap();
    let mut market = DistributionMarket::new(50.0, 21.05026039569057, initial).unwrap();
    let sigma_min = market.minimum_sigma();
    let invalid = SupportedDistribution::normal(100.0, sigma_min * 0.95).unwrap();
    let error = market.trade(invalid).unwrap_err();
    assert!(error.contains("sigma"));
}

#[test]
fn trade_moves_market_and_requires_collateral() {
    let initial = SupportedDistribution::normal(95.0, 10.0).unwrap();
    let mut market = DistributionMarket::new(50.0, 21.05026039569057, initial).unwrap();
    let collateral = market
        .trade(SupportedDistribution::normal(100.0, 10.0).unwrap())
        .unwrap();

    assert!(collateral > 0.0);
    assert_eq!(
        market.get_market_distribution(),
        SupportedDistribution::normal(100.0, 10.0).unwrap()
    );
}

#[test]
fn numerical_minimum_matches_paper_style_normal_search() {
    let old_distribution = SupportedDistribution::normal(1.5, 0.45).unwrap();
    let new_distribution = SupportedDistribution::normal(1.9, 0.4).unwrap();
    let old_f = ScaledDistribution::from_l2_target(old_distribution, 2.0).unwrap();
    let new_f = ScaledDistribution::from_l2_target(new_distribution, 2.0).unwrap();
    let range = SearchRange::new(-3.0, 6.0).unwrap();
    let minimum = find_global_minimum(&old_f, &new_f, range).unwrap();
    let mut brute_force_best_x = range.lower;
    let mut brute_force_best_value = f64::INFINITY;

    for step in 0..=20_000 {
        let x = range.lower + (range.upper - range.lower) * step as f64 / 20_000.0;
        let value = new_f.value_at(x) - old_f.value_at(x);
        if value < brute_force_best_value {
            brute_force_best_value = value;
            brute_force_best_x = x;
        }
    }

    assert_close(minimum.x_min, brute_force_best_x, 5e-3);
    assert_close(minimum.value, brute_force_best_value, 5e-4);
    assert!(minimum.value < 0.0);
    assert!(verify_minimum_onchain(minimum.x_min, &old_f, &new_f));
}

#[test]
fn lp_addition_and_removal_scale_the_market() {
    let initial = SupportedDistribution::normal(95.0, 10.0).unwrap();
    let mut market = DistributionMarket::new(50.0, 21.05026039569057, initial).unwrap();
    let starting_b = market.b;
    let starting_k = market.k;
    let starting_lambda = market.current_f.lambda;

    let new_shares = market.add_liquidity("lp_2", 0.5).unwrap();
    assert_close(new_shares, 0.5, 1e-12);
    assert_close(market.b, starting_b * 1.5, 1e-10);
    assert_close(market.k, starting_k * 1.5, 1e-10);
    assert_close(market.current_f.lambda, starting_lambda * 1.5, 1e-10);

    let removed = market.remove_liquidity("lp_2", 0.5).unwrap();
    assert_close(removed, 25.0, 1e-9);
    assert_close(market.b, starting_b, 1e-9);
    assert_close(market.k, starting_k, 1e-9);
}

#[test]
fn resolution_conserves_cash_across_traders_and_lps() {
    let initial = SupportedDistribution::normal(95.0, 10.0).unwrap();
    let mut market = DistributionMarket::new(50.0, 21.05026039569057, initial).unwrap();
    market
        .trade(SupportedDistribution::normal(100.0, 10.0).unwrap())
        .unwrap();
    market.add_liquidity("lp_2", 0.5).unwrap();

    let resolution = market.resolve(107.6).unwrap();
    let trader_total: f64 = resolution
        .trader_payouts
        .iter()
        .map(|(_, payout)| payout)
        .sum();
    let lp_total: f64 = resolution.lp_payouts.values().sum();

    assert_close(trader_total + lp_total, market.cash, 1e-8);
}

#[test]
fn zero_collateral_trade_is_detected_for_identical_distribution() {
    let initial = SupportedDistribution::normal(95.0, 10.0).unwrap();
    let market = DistributionMarket::new(50.0, 21.05026039569057, initial.clone()).unwrap();
    let old_f = market.current_f.clone();
    let new_f = ScaledDistribution::from_l2_target(initial, market.k).unwrap();
    let collateral = market.compute_collateral(&old_f, &new_f).unwrap();
    assert_close(collateral, 0.0, 1e-12);
}

#[test]
fn very_peaked_distribution_near_sigma_floor_remains_solvent() {
    let initial = SupportedDistribution::normal(0.0, 1.5).unwrap();
    let market = DistributionMarket::new(10.0, 5.0, initial).unwrap();
    let sigma = market.minimum_sigma() * 1.001;
    let candidate = SupportedDistribution::normal(0.5, sigma).unwrap();
    let f = ScaledDistribution::from_l2_target(candidate, market.k).unwrap();
    assert!(f.max_value() <= market.b + 1e-9);
}

#[test]
fn scoring_module_verifies_collateral_and_payouts() {
    let old_distribution = SupportedDistribution::normal(95.0, 10.0).unwrap();
    let new_distribution = SupportedDistribution::normal(100.0, 10.0).unwrap();
    let old_f = ScaledDistribution::from_l2_target(old_distribution, 21.05026039569057).unwrap();
    let new_f = ScaledDistribution::from_l2_target(new_distribution, 21.05026039569057).unwrap();
    let search = SearchRange::new(0.0, 200.0).unwrap();
    let minimum = find_global_minimum(&old_f, &new_f, search).unwrap();
    let collateral = (-minimum.value).max(0.0);

    assert!(collateral_is_sufficient(&old_f, &new_f, collateral, search).unwrap());

    let payout = trader_payout(&old_f, &new_f, 107.6, collateral);
    assert!(payout >= 0.0);
}

#[test]
fn uniform_distribution_basics_hold() {
    let uniform = UniformDistribution::new(-2.0, 2.0).unwrap();
    assert_close(uniform.pdf(0.0), 0.25, 1e-12);
    assert_close(uniform.cdf(-3.0), 0.0, 1e-12);
    assert_close(uniform.cdf(2.0), 1.0, 1e-12);
    assert_close(uniform.l2_norm(), 0.5, 1e-12);
}

#[test]
fn uniform_trade_collateral_captures_endpoint_minimum() {
    let old_distribution = SupportedDistribution::uniform(-2.0, 2.0).unwrap();
    let new_distribution = SupportedDistribution::uniform(0.0, 4.0).unwrap();
    let old_f = ScaledDistribution::from_l2_target(old_distribution, 2.0).unwrap();
    let new_f = ScaledDistribution::from_l2_target(new_distribution, 2.0).unwrap();
    let range = SearchRange::new(-4.0, 6.0).unwrap();
    let minimum = find_global_minimum(&old_f, &new_f, range).unwrap();

    assert!(minimum.x_min >= -2.0 - 1e-9);
    assert!(minimum.x_min <= 0.0 + 1e-9);
    assert!(minimum.value < 0.0);
}

#[test]
fn cauchy_distribution_basics_hold() {
    let cauchy = CauchyDistribution::new(1.5, 2.0).unwrap();
    assert_close(cauchy.pdf(1.5), 1.0 / (std::f64::consts::PI * 2.0), 1e-12);
    assert_close(cauchy.cdf(1.5), 0.5, 1e-12);
    assert_close(cauchy.max_pdf(), 1.0 / (std::f64::consts::PI * 2.0), 1e-12);
    assert_close(
        cauchy.l2_norm(),
        (1.0 / (4.0 * std::f64::consts::PI)).sqrt(),
        1e-12,
    );
}

#[test]
fn student_t_distribution_basics_hold() {
    let student_t = StudentTDistribution::new(0.0, 1.0, 4.0).unwrap();
    let expected_center = 0.375;
    assert_close(student_t.pdf(0.0), expected_center, 1e-12);
    assert_close(student_t.cdf(0.0), 0.5, 1e-12);
    assert!(student_t.max_pdf() > 0.0);
    assert!(student_t.l2_norm() > 0.0);
}

#[test]
fn student_t_large_nu_approaches_normal_center_density() {
    let student_t = StudentTDistribution::new(0.0, 1.0, 100.0).unwrap();
    let normal = NormalDistribution::new(0.0, 1.0).unwrap();
    assert_close(student_t.pdf(0.0), normal.pdf(0.0), 5e-3);
}

#[test]
fn cauchy_market_trade_requires_positive_collateral() {
    let initial = SupportedDistribution::cauchy(0.0, 2.0).unwrap();
    let mut market = DistributionMarket::new(10.0, 3.0, initial).unwrap();
    let collateral = market
        .trade(SupportedDistribution::cauchy(1.5, 2.0).unwrap())
        .unwrap();
    assert!(collateral > 0.0);
}

#[test]
fn student_t_market_trade_requires_positive_collateral() {
    let initial = SupportedDistribution::student_t(0.0, 1.25, 5.0).unwrap();
    let mut market = DistributionMarket::new(10.0, 3.0, initial).unwrap();
    let collateral = market
        .trade(SupportedDistribution::student_t(1.0, 1.25, 5.0).unwrap())
        .unwrap();
    assert!(collateral > 0.0);
}
