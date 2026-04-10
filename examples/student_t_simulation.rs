use distribution_markets::{find_scenario, render_report, run_scenario};

fn main() -> Result<(), String> {
    let scenario =
        find_scenario("student_t").ok_or_else(|| "student_t scenario missing".to_string())?;
    let report = run_scenario(&scenario)?;
    println!("{}", render_report(&report));
    Ok(())
}
