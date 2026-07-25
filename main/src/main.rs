//! Runs a world and narrates it.
//!
//! The simulation records structured events; this is the only place that turns them
//! into sentences. Everything printed here is a *view* of the chronicle, which is the
//! shape the observer API will grow into.

use std::process::ExitCode;

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

mod render;

const USAGE: &str = "\
life-rs — run a world and watch it

usage: cargo run -p main -- [options]

options:
  --seed <hex>     replay a specific world (default: a new one)
  --days <n>       how many days to simulate      (default: 3)
  --people <n>     how many inhabitants           (default: 1)
  --years <n>      simulate years instead of days
  --min <level>    least important event to show  (default: routine)
                   routine | notable | pivotal | historic | epochal
  --dossier        end with a close look at one random person
  --balance        report what the run turned out to be about
  --quiet          print only the closing summary
  -h, --help       this message
";

struct Options {
    seed: WorldSeed,
    span: Duration,
    span_label: String,
    people: usize,
    min_salience: Salience,
    dossier: bool,
    balance: bool,
    quiet: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            seed: WorldSeed::from_entropy(),
            span: Duration::from_days(3),
            span_label: "3 days".to_string(),
            people: 1,
            min_salience: Salience::Routine,
            dossier: false,
            balance: false,
            quiet: false,
        }
    }
}

fn main() -> ExitCode {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => {
            print!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let mut world = World::genesis(options.seed, options.people);
    // Don't record what won't be shown. A century of one person's every meal is tens of
    // millions of records; asking to see only the pivotal moments should also mean not
    // paying to store the rest.
    world.record_only(options.min_salience);
    let started = world.now();
    world.run_for(options.span);

    // The seed is the world's name. Printed first and last so it survives a scrollback
    // that has lost the top of the run.
    println!("world {}", options.seed);
    println!(
        "{} {} on {}, {}\n",
        options.people,
        if options.people == 1 {
            "person"
        } else {
            "people"
        },
        world
            .planets
            .iter()
            .next()
            .map(|(_, p)| p.name.as_str())
            .unwrap_or("nowhere"),
        options.span_label,
    );

    if !options.quiet {
        for record in world.chronicle.at_least(options.min_salience) {
            println!("{}", render::line(&world, record));
        }
        println!();
    }

    if world.places.len() > 1 {
        println!("── neighbourhoods ──");
        for line in render::neighbourhoods(&world) {
            println!("{line}");
        }
        println!();
    }

    if options.balance {
        println!("── inheritance and circumstance ──");
        println!("{}", observer::measure(&world));
        println!();
    }

    if options.dossier {
        print_dossier(&mut world);
    }

    println!(
        "{} events over {}. {} of {} still living. replay with --seed {}",
        world.chronicle.len(),
        world.now().since(started),
        world.living(),
        world.people.len(),
        options.seed,
    );

    ExitCode::SUCCESS
}

/// A close look at one person — the omniscient view, in its earliest form.
fn print_dossier(world: &mut World) {
    let living: Vec<_> = world
        .people
        .iter()
        .filter(|(_, p)| p.is_alive())
        .map(|(id, _)| id)
        .collect();

    let mut rng = world.stream(sim_core::Domain::Chance);
    let Some(&id) = rng.pick(&living) else {
        println!("nobody left to look at.\n");
        return;
    };

    println!("── a closer look ──");
    println!("{}", render::portrait(world, id));

    println!("\nfamily:");
    for line in render::family(world, id) {
        println!("{line}");
    }

    println!("\nwhere their temperament came from:");
    for line in render::heritage(world, id) {
        println!("{line}");
    }

    println!("\nwhy they are doing that:");
    for line in render::reasoning(world, id) {
        println!("{line}");
    }
    println!();
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Option<Options>, String> {
    let mut options = Options::default();
    let mut args = args.peekable();

    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--quiet" => options.quiet = true,
            "--dossier" => options.dossier = true,
            "--balance" => options.balance = true,
            "--seed" => {
                let raw = value()?;
                options.seed =
                    WorldSeed::parse(&raw).map_err(|e| format!("bad seed {raw:?}: {e}"))?;
            }
            "--days" => {
                let raw = value()?;
                let days: u64 = raw
                    .parse()
                    .map_err(|e| format!("bad day count {raw:?}: {e}"))?;
                options.span = Duration::from_days(days);
                options.span_label = format!("{days} days");
            }
            "--years" => {
                let raw = value()?;
                let years: u64 = raw
                    .parse()
                    .map_err(|e| format!("bad year count {raw:?}: {e}"))?;
                options.span = Duration::from_years(years);
                options.span_label = format!("{years} years");
            }
            "--people" => {
                let raw = value()?;
                options.people = raw
                    .parse()
                    .map_err(|e| format!("bad population {raw:?}: {e}"))?;
            }
            "--min" => {
                let raw = value()?;
                options.min_salience = parse_salience(&raw)?;
            }
            other => return Err(format!("unknown option {other:?}")),
        }
    }

    Ok(Some(options))
}

fn parse_salience(text: &str) -> Result<Salience, String> {
    match text.to_ascii_lowercase().as_str() {
        "routine" => Ok(Salience::Routine),
        "notable" => Ok(Salience::Notable),
        "pivotal" => Ok(Salience::Pivotal),
        "historic" => Ok(Salience::Historic),
        "epochal" => Ok(Salience::Epochal),
        other => Err(format!("unknown salience {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_are_a_short_run_of_a_new_world() {
        let options = parse(&[]).unwrap().unwrap();
        assert_eq!(options.span, Duration::from_days(3));
        assert_eq!(options.people, 1);
        assert_eq!(options.min_salience, Salience::Routine);
        assert!(!options.quiet);
    }

    #[test]
    fn a_seed_can_be_named_to_replay_a_world() {
        let options = parse(&["--seed", "0xff", "--days", "10", "--people", "4"])
            .unwrap()
            .unwrap();
        assert_eq!(options.seed, WorldSeed::from_u128(0xff));
        assert_eq!(options.span, Duration::from_days(10));
        assert_eq!(options.people, 4);
    }

    #[test]
    fn a_span_can_be_given_in_years() {
        let options = parse(&["--years", "50"]).unwrap().unwrap();
        assert_eq!(options.span, Duration::from_years(50));
        assert_eq!(options.span_label, "50 years");
    }

    #[test]
    fn the_dossier_is_opt_in() {
        assert!(!parse(&[]).unwrap().unwrap().dossier);
        assert!(parse(&["--dossier"]).unwrap().unwrap().dossier);
    }

    #[test]
    fn the_balance_report_is_opt_in() {
        assert!(!parse(&[]).unwrap().unwrap().balance);
        assert!(parse(&["--balance"]).unwrap().unwrap().balance);
    }

    #[test]
    fn help_short_circuits() {
        assert!(parse(&["--help"]).unwrap().is_none());
        assert!(parse(&["-h"]).unwrap().is_none());
    }

    #[test]
    fn bad_input_is_reported_not_ignored() {
        assert!(parse(&["--days", "soon"]).is_err());
        assert!(parse(&["--years", "lots"]).is_err());
        assert!(parse(&["--seed", "zzz"]).is_err());
        assert!(parse(&["--min", "loud"]).is_err());
        assert!(parse(&["--nonsense"]).is_err());
        assert!(parse(&["--days"]).is_err(), "a missing value is an error");
    }

    #[test]
    fn salience_levels_parse_case_insensitively() {
        assert_eq!(parse_salience("Pivotal").unwrap(), Salience::Pivotal);
        assert_eq!(parse_salience("EPOCHAL").unwrap(), Salience::Epochal);
    }
}
