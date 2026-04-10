use distribution_markets::{find_scenario, render_report, run_scenario};

fn main() -> Result<(), String> {
    let scenario = find_scenario("normal").ok_or_else(|| "normal scenario missing".to_string())?;
    let report = run_scenario(&scenario)?;
    println!("{}", render_report(&report));
    Ok(())
}
