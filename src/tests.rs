use crate::distributions::{
    CauchyDistribution, Distribution, NormalDistribution, ScaledDistribution, StudentTDistribution,
    SupportedDistribution, UniformDistribution,
};
use crate::fixed_point::Fixed;
use crate::market::DistributionMarket;
use crate::normal_market::FixedNormalMarket;
use crate::normal_math::{
    FixedNormalDistribution, fixed_calculate_f, fixed_calculate_lambda, fixed_calculate_maximum_k,
    fixed_calculate_minimum_sigma, fixed_required_collateral, fixed_required_collateral_quote,
};
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

#[test]
fn fixed_normal_lambda_matches_solidity_reference_case() {
    let sigma = Fixed::from_f64(10.0).unwrap();
    let k = Fixed::from_f64(100.0).unwrap();
    let lambda = fixed_calculate_lambda(sigma, k).unwrap();
    assert_close(lambda.to_f64(), 595.391274861, 1e-6);
}

#[test]
fn fixed_normal_f_matches_solidity_reference_cases() {
    let distribution = FixedNormalDistribution::new(
        Fixed::from_f64(100.0).unwrap(),
        Fixed::from_f64(10.0).unwrap(),
    )
    .unwrap();
    let k = Fixed::from_f64(100.0).unwrap();
    let at_mean = fixed_calculate_f(Fixed::from_f64(100.0).unwrap(), distribution, k).unwrap();
    let off_mean = fixed_calculate_f(Fixed::from_f64(85.0).unwrap(), distribution, k).unwrap();

    assert_close(at_mean.to_f64(), 23.75268, 1e-5);
    assert_close(off_mean.to_f64(), 7.71136, 1e-5);
}

#[test]
fn fixed_normal_minimum_sigma_and_maximum_k_round_trip() {
    let k = Fixed::from_f64(100.0).unwrap();
    let b = Fixed::from_f64(100.0).unwrap();
    let min_sigma = fixed_calculate_minimum_sigma(k, b).unwrap();
    let max_k = fixed_calculate_maximum_k(min_sigma, b).unwrap();
    assert_close(max_k.to_f64(), 100.0, 1e-5);
}

#[test]
fn fixed_normal_required_collateral_matches_solidity_reference_case() {
    let from = FixedNormalDistribution::new(
        Fixed::from_f64(1.5).unwrap(),
        Fixed::from_f64(0.45).unwrap(),
    )
    .unwrap();
    let to =
        FixedNormalDistribution::new(Fixed::from_f64(1.9).unwrap(), Fixed::from_f64(0.4).unwrap())
            .unwrap();
    let collateral = fixed_required_collateral(from, to, Fixed::from_f64(2.0).unwrap()).unwrap();
    assert_close(collateral.to_f64(), 1.175948, 5e-3);
}

#[test]
fn fixed_normal_market_initializes_with_expected_state() {
    let market = FixedNormalMarket::new(
        Fixed::from_f64(50.0).unwrap(),
        Fixed::from_f64(21.05026039569057).unwrap(),
        FixedNormalDistribution::new(
            Fixed::from_f64(95.0).unwrap(),
            Fixed::from_f64(10.0).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    assert_close(market.b.to_f64(), 50.0, 1e-9);
    assert_close(market.k.to_f64(), 21.05026039569057, 1e-6);
    assert_close(market.current_lambda.to_f64(), 125.331413731, 1e-6);
}

#[test]
fn fixed_normal_market_trade_matches_float_normal_scenario() {
    let mut fixed_market = FixedNormalMarket::new(
        Fixed::from_f64(50.0).unwrap(),
        Fixed::from_f64(21.05026039569057).unwrap(),
        FixedNormalDistribution::new(
            Fixed::from_f64(95.0).unwrap(),
            Fixed::from_f64(10.0).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut float_market = DistributionMarket::new(
        50.0,
        21.05026039569057,
        SupportedDistribution::normal(95.0, 10.0).unwrap(),
    )
    .unwrap();

    let fixed_collateral = fixed_market
        .trade(
            FixedNormalDistribution::new(
                Fixed::from_f64(100.0).unwrap(),
                Fixed::from_f64(10.0).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    let float_collateral = float_market
        .trade(SupportedDistribution::normal(100.0, 10.0).unwrap())
        .unwrap();

    assert_close(fixed_collateral.to_f64(), 1.485192826, 1e-5);
    assert_close(fixed_collateral.to_f64(), float_collateral, 1e-6);
}

#[test]
fn fixed_normal_market_liquidity_scales_state() {
    let mut market = FixedNormalMarket::new(
        Fixed::from_f64(50.0).unwrap(),
        Fixed::from_f64(21.05026039569057).unwrap(),
        FixedNormalDistribution::new(
            Fixed::from_f64(95.0).unwrap(),
            Fixed::from_f64(10.0).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let minted = market
        .add_liquidity("lp_2", Fixed::from_f64(0.5).unwrap())
        .unwrap();

    assert_close(minted.to_f64(), 0.5, 1e-9);
    assert_close(market.b.to_f64(), 75.0, 1e-6);
    assert_close(market.k.to_f64(), 31.5753905935, 1e-6);
}

#[test]
fn fixed_normal_market_resolves_consistently() {
    let mut market = FixedNormalMarket::new(
        Fixed::from_f64(50.0).unwrap(),
        Fixed::from_f64(21.05026039569057).unwrap(),
        FixedNormalDistribution::new(
            Fixed::from_f64(95.0).unwrap(),
            Fixed::from_f64(10.0).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    market
        .trade(
            FixedNormalDistribution::new(
                Fixed::from_f64(100.0).unwrap(),
                Fixed::from_f64(10.0).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    market
        .add_liquidity("lp_2", Fixed::from_f64(0.5).unwrap())
        .unwrap();

    let resolution = market.resolve(Fixed::from_f64(107.6).unwrap()).unwrap();
    let trader_total: f64 = resolution
        .trader_payouts
        .iter()
        .map(|(_, payout)| payout.to_f64())
        .sum();
    let lp_total: f64 = resolution
        .lp_payouts
        .values()
        .map(|value| value.to_f64())
        .sum();

    assert_close(trader_total + lp_total, market.cash.to_f64(), 1e-6);
}

#[test]
fn fixed_normal_quote_verifier_accepts_fresh_quote_and_rejects_stale_quote() {
    let mut market = FixedNormalMarket::new(
        Fixed::from_f64(50.0).unwrap(),
        Fixed::from_f64(21.05026039569057).unwrap(),
        FixedNormalDistribution::new(
            Fixed::from_f64(95.0).unwrap(),
            Fixed::from_f64(10.0).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let fresh_quote = market
        .quote_trade(
            FixedNormalDistribution::new(
                Fixed::from_f64(100.0).unwrap(),
                Fixed::from_f64(10.0).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    assert!(market.verify_trade_quote(&fresh_quote).is_ok());

    market.trade_with_quote(fresh_quote.clone()).unwrap();
    assert!(market.verify_trade_quote(&fresh_quote).is_err());
}

#[test]
fn fixed_normal_market_remove_liquidity_restores_state_after_add() {
    let mut market = FixedNormalMarket::new(
        Fixed::from_f64(50.0).unwrap(),
        Fixed::from_f64(21.05026039569057).unwrap(),
        FixedNormalDistribution::new(
            Fixed::from_f64(95.0).unwrap(),
            Fixed::from_f64(10.0).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let minted = market
        .add_liquidity("lp_2", Fixed::from_f64(0.5).unwrap())
        .unwrap();
    let removed = market.remove_liquidity("lp_2", minted).unwrap();

    assert_close(removed.to_f64(), 25.0, 1e-6);
    assert_close(market.b.to_f64(), 50.0, 1e-6);
    assert_close(market.k.to_f64(), 21.05026039569057, 1e-6);
}

#[test]
fn fixed_normal_market_handles_repeated_trades() {
    let mut market = FixedNormalMarket::new(
        Fixed::from_f64(50.0).unwrap(),
        Fixed::from_f64(21.05026039569057).unwrap(),
        FixedNormalDistribution::new(
            Fixed::from_f64(95.0).unwrap(),
            Fixed::from_f64(10.0).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let targets = [97.5, 100.0, 103.0, 99.0, 104.0];
    for target in targets {
        let collateral = market
            .trade(
                FixedNormalDistribution::new(
                    Fixed::from_f64(target).unwrap(),
                    Fixed::from_f64(10.0).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        assert!(collateral.raw() > 0);
    }

    assert_eq!(market.trades.len(), targets.len());
}

#[test]
fn fixed_normal_market_handles_extreme_mean_shift() {
    let mut market = FixedNormalMarket::new(
        Fixed::from_f64(50.0).unwrap(),
        Fixed::from_f64(21.05026039569057).unwrap(),
        FixedNormalDistribution::new(
            Fixed::from_f64(95.0).unwrap(),
            Fixed::from_f64(10.0).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let collateral = market
        .trade(
            FixedNormalDistribution::new(
                Fixed::from_f64(140.0).unwrap(),
                Fixed::from_f64(10.0).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();

    assert!(collateral.raw() > 0);
    assert!(market.cash.raw() > market.b.raw());
}

#[test]
fn fixed_normal_market_accepts_sigma_near_floor() {
    let market = FixedNormalMarket::new(
        Fixed::from_f64(10.0).unwrap(),
        Fixed::from_f64(5.0).unwrap(),
        FixedNormalDistribution::new(
            Fixed::from_f64(0.0).unwrap(),
            Fixed::from_f64(1.5).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let sigma = market.minimum_sigma().unwrap() + Fixed::from_f64(0.001).unwrap();
    let quote = market
        .quote_trade(
            FixedNormalDistribution::new(Fixed::from_f64(0.5).unwrap(), sigma).unwrap(),
        )
        .unwrap();

    assert!(quote.collateral_quote.collateral_required.raw() >= 0);
}

#[test]
fn fixed_normal_market_repeated_liquidity_changes_preserve_positive_state() {
    let mut market = FixedNormalMarket::new(
        Fixed::from_f64(50.0).unwrap(),
        Fixed::from_f64(21.05026039569057).unwrap(),
        FixedNormalDistribution::new(
            Fixed::from_f64(95.0).unwrap(),
            Fixed::from_f64(10.0).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    let a = market
        .add_liquidity("lp_2", Fixed::from_f64(0.25).unwrap())
        .unwrap();
    let b = market
        .add_liquidity("lp_3", Fixed::from_f64(0.10).unwrap())
        .unwrap();
    market.remove_liquidity("lp_2", a.div_int(2)).unwrap();
    market.remove_liquidity("lp_3", b.div_int(2)).unwrap();

    assert!(market.b.raw() > 0);
    assert!(market.k.raw() > 0);
    assert!(market.current_lambda.raw() > 0);
    assert!(market.total_lp_shares.raw() > 0);
}

#[test]
fn fixed_normal_market_full_resolution_conservation_under_stress() {
    let mut market = FixedNormalMarket::new(
        Fixed::from_f64(50.0).unwrap(),
        Fixed::from_f64(21.05026039569057).unwrap(),
        FixedNormalDistribution::new(
            Fixed::from_f64(95.0).unwrap(),
            Fixed::from_f64(10.0).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();

    for target in [97.5, 100.0, 102.0, 98.0] {
        market
            .trade(
                FixedNormalDistribution::new(
                    Fixed::from_f64(target).unwrap(),
                    Fixed::from_f64(10.0).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let minted = market
        .add_liquidity("lp_2", Fixed::from_f64(0.30).unwrap())
        .unwrap();
    market.remove_liquidity("lp_2", minted.div_int(3)).unwrap();

    let resolution = market.resolve(Fixed::from_f64(101.25).unwrap()).unwrap();
    let trader_total: i128 = resolution
        .trader_payouts
        .iter()
        .map(|(_, payout)| payout.raw())
        .sum();
    let lp_total: i128 = resolution.lp_payouts.values().map(|value| value.raw()).sum();

    assert_eq!(
        trader_total + lp_total + resolution.cash_remaining.raw(),
        market.cash.raw()
    );
}

#[test]
fn fixed_normal_collateral_quote_exposes_bounded_verifier_metadata() {
    let from = FixedNormalDistribution::new(
        Fixed::from_f64(95.0).unwrap(),
        Fixed::from_f64(10.0).unwrap(),
    )
    .unwrap();
    let to = FixedNormalDistribution::new(
        Fixed::from_f64(100.0).unwrap(),
        Fixed::from_f64(10.0).unwrap(),
    )
    .unwrap();
    let quote = fixed_required_collateral_quote(from, to, Fixed::from_f64(21.05026039569057).unwrap()).unwrap();

    assert!(quote.collateral_required.raw() > 0);
    assert!(quote.upper_bound.raw() > quote.lower_bound.raw());
    assert!(quote.coarse_samples > 0);
    assert!(quote.refine_samples > 0);
}
