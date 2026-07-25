//! Runs a world and narrates it.
//!
//! The simulation records structured events; this is the only place that turns them
//! into sentences. Everything printed here is a *view* of the chronicle, which is the
//! shape the observer API will grow into.

use std::process::ExitCode;

use sim::World;
use sim_core::{Domain, Duration, Salience, WorldSeed};

mod export;
mod globe;
mod render;

/// The viewer page, with a placeholder where the world goes.
const VIEWER: &str = include_str!("viewer.html");
/// The deep-time page, with a placeholder where the planet goes.
const GLOBE: &str = include_str!("globe.html");

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
  --detail <n>     how many people to simulate finely (default: 400)
  --json           write the whole world out as JSON
  --html           write a self-contained page you can open in a browser
  --globe <myr>    run the solid planet for this many megayears instead, and
                   write a page you can scrub through
  --grid <level>   how fine the planet's grid is (default: 4, ~450 km cells)
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
    detail: Option<usize>,
    json: bool,
    html: bool,
    globe: Option<f64>,
    grid: u8,
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
            detail: None,
            json: false,
            html: false,
            globe: None,
            grid: 4,
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

    // The solid planet is its own thing: no people, no chronicle, and a clock that runs
    // a megayear at a time rather than fifteen minutes.
    if let Some(myr) = options.globe {
        println!("{}", run_globe(&options, myr));
        return ExitCode::SUCCESS;
    }

    let mut world = World::genesis(options.seed, options.people);
    // Don't record what won't be shown. A century of one person's every meal is tens of
    // millions of records; asking to see only the pivotal moments should also mean not
    // paying to store the rest.
    world.record_only(options.min_salience);
    if let Some(budget) = options.detail {
        world.set_detail_budget(budget);
    }
    let started = world.now();

    // The JSON path walks the run a year at a time so the viewer has a series to plot;
    // otherwise it is one jump to the horizon.
    let mut series = Vec::new();
    if options.json || options.html {
        let years = options.span.as_years().ceil() as u64;
        for year in 0..=years {
            if year > 0 {
                world.run_for(Duration::from_years(1));
            }
            series.push(export::YearSample {
                year,
                living: world.living(),
                affluence: world.places.iter().map(|(_, p)| p.env.affluence).collect(),
            });
        }
        let balance = observer::measure(&world);
        let data = export::snapshot(&world, &series, &balance);
        if options.html {
            // The viewer is a template with one hole in it, filled at run time. Keeping
            // it a template rather than a written-out file means any seed can produce
            // its own page.
            println!("{}", VIEWER.replace("__WORLD_DATA__", &data));
        } else {
            println!("{data}");
        }
        return ExitCode::SUCCESS;
    }

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

/// Run the lithosphere and render its history.
///
/// Sampled at a fixed number of points rather than every step: a page carrying five
/// hundred snapshots of a forty-thousand-cell grid is a hundred megabytes, and the eye
/// cannot tell twenty frames from five hundred when each one is a different continent
/// anyway.
fn run_globe(options: &Options, myr: f64) -> String {
    const FRAMES: usize = 11;
    const STEP_MYR: f32 = 4.0;
    /// Where in the star's life the run starts, in gigayears. Three and a half puts the
    /// sun at about nine tenths of today's output, so a gigayear of running carries it
    /// across the part of the main sequence the thermostat has to cope with.
    const START_GYR: f64 = 3.5;

    let mut rng = options.seed.stream(Domain::Terrain, 0, 0);
    let mut planet = geo::Lithosphere::genesis(options.grid, 9, 0.42, &mut rng);
    // One step first, so the plate boundaries exist and there is volcanism to feed the
    // carbon cycle before the climate is asked to settle against it.
    planet.step_myr(STEP_MYR, &mut rng);
    let mut climate =
        climate::Climate::genesis(&planet, START_GYR, climate::insolation::EARTH_OBLIQUITY);
    // The one wire that runs back down the stack: rivers cut in proportion to how much
    // falls on them, and only the climate knows that.
    let mut runoff: Vec<f32> = Vec::with_capacity(planet.grid().len());
    let soak = |planet: &geo::Lithosphere, climate: &climate::Climate, into: &mut Vec<f32>| {
        into.clear();
        into.extend(
            planet
                .grid()
                .cells()
                .map(|c| climate.rain_mm(c) / climate::moisture::REFERENCE_RAIN_MM),
        );
    };
    soak(&planet, &climate, &mut runoff);
    planet.set_runoff(&runoff);

    let mut life = biome::Biosphere::read(&planet, &climate);
    let mut fauna = ecology::Ecology::genesis(&planet, &life, &climate, 48, options.seed);
    // A few short steps so the populations find their level before the first frame.
    for _ in 0..5 {
        fauna.step_myr(&planet, &life, &climate, 1.0, &mut rng);
    }
    let mut frames = vec![globe::sample(&planet, &climate, &life, &fauna)];
    let per_frame = myr / (FRAMES - 1) as f64;
    for _ in 1..FRAMES {
        let mut done = 0.0;
        while done < per_frame {
            let step = STEP_MYR.min((per_frame - done) as f32);
            planet.step_myr(step, &mut rng);
            climate.step_myr(&planet, step, &mut rng);
            soak(&planet, &climate, &mut runoff);
            planet.set_runoff(&runoff);
            life = biome::Biosphere::read(&planet, &climate);
            fauna.step_myr(&planet, &life, &climate, step, &mut rng);
            done += step as f64;
        }
        frames.push(globe::sample(&planet, &climate, &life, &fauna));
    }

    globe::page(GLOBE, &options.seed.to_string(), options.grid, &frames)
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
            "--json" => options.json = true,
            "--html" => options.html = true,
            "--globe" => {
                let raw = value()?;
                let myr: f64 = raw
                    .parse()
                    .map_err(|e| format!("bad megayear count {raw:?}: {e}"))?;
                // NaN included: `myr > 0.0` is false for it, which is what we want.
                if !myr.is_finite() || myr <= 0.0 {
                    return Err("--globe needs a positive number of megayears".to_string());
                }
                options.globe = Some(myr);
            }
            "--grid" => {
                let raw = value()?;
                let level: u8 = raw
                    .parse()
                    .map_err(|e| format!("bad grid level {raw:?}: {e}"))?;
                if !(2..=6).contains(&level) {
                    return Err(format!(
                        "grid level {level} is outside 2..=6; below four the plates weld \
                         into one and never rift again, and above six a run takes minutes"
                    ));
                }
                options.grid = level;
            }
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
            "--detail" => {
                let raw = value()?;
                options.detail = Some(
                    raw.parse()
                        .map_err(|e| format!("bad detail budget {raw:?}: {e}"))?,
                );
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
    fn the_viewer_template_has_exactly_one_hole() {
        assert_eq!(
            VIEWER.matches("__WORLD_DATA__").count(),
            1,
            "the world is substituted in exactly once"
        );
        assert!(VIEWER.contains("<title>"), "a page needs a name");
        assert!(
            !VIEWER.contains("http://") && !VIEWER.contains("https://"),
            "the page must be self-contained — no external fetches"
        );
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
