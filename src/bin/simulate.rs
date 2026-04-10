use distribution_markets::{builtin_scenarios, find_scenario, render_report, run_scenario};
use std::env;
use std::io::{self, Write};

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        return run_interactive();
    }

    match args[0].as_str() {
        "list" => {
            print_scenarios();
            Ok(())
        }
        "inspect" => {
            let slug = args
                .get(1)
                .ok_or_else(|| "usage: simulate inspect <scenario>".to_string())?;
            inspect_scenario(slug)
        }
        "run" => {
            let slug = args
                .get(1)
                .ok_or_else(|| "usage: simulate run <scenario> [outcome]".to_string())?;
            let mut scenario =
                find_scenario(slug).ok_or_else(|| format!("unknown scenario: {slug}"))?;
            if let Some(outcome) = args.get(2) {
                scenario.outcome = outcome
                    .parse::<f64>()
                    .map_err(|_| format!("invalid outcome: {outcome}"))?;
            }
            let report = run_scenario(&scenario)?;
            println!("{}", render_report(&report));
            Ok(())
        }
        _ => Err("usage: simulate [list|inspect <scenario>|run <scenario> [outcome]]".to_string()),
    }
}

fn run_interactive() -> Result<(), String> {
    let scenarios = builtin_scenarios();
    println!("Interactive Distribution Market Simulation");
    println!("=========================================");
    println!();
    println!("Available scenarios:");
    for (index, scenario) in scenarios.iter().enumerate() {
        println!("  {}. {} ({})", index + 1, scenario.title, scenario.slug);
    }
    println!();
    print!("Choose a scenario by number: ");
    io::stdout().flush().map_err(|error| error.to_string())?;

    let mut choice = String::new();
    io::stdin()
        .read_line(&mut choice)
        .map_err(|error| error.to_string())?;
    let selected_index = choice
        .trim()
        .parse::<usize>()
        .map_err(|_| "please enter a valid scenario number".to_string())?;
    let mut scenario = scenarios
        .get(selected_index.saturating_sub(1))
        .cloned()
        .ok_or_else(|| "scenario selection out of range".to_string())?;

    print!(
        "Optional custom outcome (press Enter to use {:.4}): ",
        scenario.outcome
    );
    io::stdout().flush().map_err(|error| error.to_string())?;

    let mut outcome_input = String::new();
    io::stdin()
        .read_line(&mut outcome_input)
        .map_err(|error| error.to_string())?;
    let trimmed = outcome_input.trim();
    if !trimmed.is_empty() {
        scenario.outcome = trimmed
            .parse::<f64>()
            .map_err(|_| "custom outcome must be a number".to_string())?;
    }

    let report = run_scenario(&scenario)?;
    println!();
    println!("{}", render_report(&report));
    Ok(())
}

fn print_scenarios() {
    println!("Built-in simulation scenarios:");
    for scenario in builtin_scenarios() {
        println!("  {:<10} {}", scenario.slug, scenario.title);
    }
}

fn inspect_scenario(slug: &str) -> Result<(), String> {
    let scenario = find_scenario(slug).ok_or_else(|| format!("unknown scenario: {slug}"))?;
    println!("Scenario: {}", scenario.slug);
    println!("Title: {}", scenario.title);
    println!("Initial backing: {:.4}", scenario.initial_backing);
    println!("Initial invariant (k): {:.6}", scenario.initial_k);
    println!("Default outcome: {:.4}", scenario.outcome);
    println!("Steps: {}", scenario.steps.len());
    Ok(())
}
