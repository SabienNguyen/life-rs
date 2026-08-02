//! Four questions asked of a world, and answered with numbers.
//!
//! Every other instrument here asks whether one mechanism works. This asks whether the *world*
//! does, against four things a living world ought to have, and it is deliberately blunt: each
//! criterion gets a measurement and a verdict, and the verdict is allowed to be no.
//!
//! - **Do souls truly interact?** Not whether ties exist — they always have — but whether people
//!   do things to each other that leave a mark, and how much of a life that amounts to.
//! - **Do regions have relationships?** §32.2 found conquest keyed on adjacent countries in a
//!   world with **zero** adjacent cross-country pairs. Countries have existed here since §24 and
//!   it has never been established that any two of them have ever had anything to do with each
//!   other.
//! - **Is the communication intelligent?** There is no language in this model and §40 says so.
//!   What there is, is `hearsay` — regard drifting toward a friend's. The question that can be
//!   answered is whether anything actually *travels*: does what a person is thought of reach
//!   people who have never met them, and does it arrive intact or as noise?
//! - **Does the world progress?** §29's advances happen. Whether they accumulate into a
//!   trajectory, or trickle at a constant rate forever against §21's ceiling, is a different
//!   question and needs centuries rather than the usual ninety years.
//!
//! Run long, because three of the four are about things that take longer than a lifetime:
//!
//!     YEARS=300 cargo run --release --example does_it_live

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

const ALL_SEEDS: [u128; 3] = [0x11, 0x21, 0x221];

fn main() {
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);
    // §48.4's switch: whether somebody's own standing buys them time to think, or only the
    // average of the quarter they live in does.
    let own_means = std::env::var("OWN_MEANS").map(|v| v != "0").unwrap_or(true);
    let founders: usize = std::env::var("FOUNDERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    // Two hundred years at three hundred founders is three quarters of an hour for three
    // seeds. `SEEDS=1` is for diagnosing rather than concluding.
    let how_many: usize = std::env::var("SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3)
        .clamp(1, ALL_SEEDS.len());
    let seeds = &ALL_SEEDS[..how_many];

    // Progress needs the world sampled *as it goes*, not at the end, so the worlds are run in
    // slices and read between them. Everything else is read once at the finish.
    let steps = 5;
    let slice = years / steps as u64;
    let mut over_time: Vec<Vec<(u64, usize, f32, usize, usize, usize, f32)>> = Vec::new();
    let mut worlds: Vec<(u128, World)> = Vec::new();
    for seed in seeds.iter().copied() {
        let mut world = World::genesis(WorldSeed::from_u128(seed), founders);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.people_think_on_their_own_means = own_means;
        let mut track = Vec::new();
        for step in 1..=steps {
            world.run_for(Duration::from_years(slice));
            let advances = world
                .chronicle
                .iter()
                .filter(|r| matches!(r.kind, sim::Happening::PersonWorksItOut { .. }))
                .count();
            let technique: f32 = world
                .places
                .ids()
                .map(|id| world.technique_of(id).level())
                .sum::<f32>()
                / world.places.ids().count().max(1) as f32;
            // What the gate in `advances` actually sees. `slack` is `prosperity - want` per
            // place and the loop does `if slack <= 0.0 { continue }` — so the question that
            // decides whether progress is throttled or *stopped* is how many places still
            // have any, not how much they have on average.
            let (mut with_slack, mut places, mut total_slack) = (0usize, 0usize, 0.0f32);
            for id in world.places.ids() {
                if world.society.households_in(id).count() == 0 {
                    continue;
                }
                places += 1;
                let slack = world
                    .places
                    .get(id)
                    .map(|p| (p.prosperity - p.want).clamp(0.0, 1.0))
                    .unwrap_or(0.0);
                total_slack += slack;
                if slack > 0.0 {
                    with_slack += 1;
                }
            }
            track.push((
                step as u64 * slice,
                advances,
                technique,
                world.living(),
                with_slack,
                places,
                total_slack / places.max(1) as f32,
            ));
        }
        over_time.push(track);
        worlds.push((seed, world));
    }

    println!("{} seeds, {years} years, {founders} founders\n", seeds.len());

    // ── 1. Do souls truly interact? ───────────────────────────────────────────────────────
    //
    // The honest unit is a life, not a world: "1,564 gifts" sounds like a society and is
    // fourteen acts per adult per lifetime or one every six years, and only one of those two
    // sentences tells you what it is like to live there.
    println!("── 1. Do souls truly interact? ──\n");
    let (mut adults, mut acted_on, mut acts, mut marked) = (0usize, 0usize, 0usize, 0usize);
    for (_, world) in &worlds {
        let now = world.now();
        for (id, person) in world.people.iter() {
            if !person.is_alive() || !person.has_matured() {
                continue;
            }
            adults += 1;
            // What their memory actually holds of other people doing things to them.
            let held = person.held();
            let borne: f32 = [
                person::memory::What::Robbed,
                person::memory::What::Wronged,
                person::memory::What::Carried,
                person::memory::What::TakenUp,
                person::memory::What::DidWrong,
            ]
            .into_iter()
            .map(|what| held.holds_of(what, now))
            .sum();
            if borne > 0.05 {
                marked += 1;
            }
        }
        for act in person::acts::Toward::ALL {
            acts += world.acted[act as usize] as usize;
        }
        acted_on += world.occasions as usize;
    }
    println!("  {adults} grown souls across the worlds");
    println!(
        "  {acts} deliberate acts — {:.1} per adult now living",
        acts as f32 / adults.max(1) as f32
    );
    println!(
        "  {marked} of them ({:.0}%) carry a memory of something somebody did to them",
        100.0 * marked as f32 / adults.max(1) as f32
    );
    println!(
        "  {acted_on} evenings spent in somebody's company; an act on {:.1}% of them",
        100.0 * acts as f32 / acted_on.max(1) as f32
    );

    // ── 2. Do regions have relationships? ─────────────────────────────────────────────────
    //
    // §32.2's finding was that countries in this world are *by construction* out of each
    // other's reach — so this asks the prior question rather than looking for a war. Do ties
    // cross a place at all; do they cross a country; does anybody move between them.
    println!("\n── 2. Do regions have relationships? ──\n");
    for (seed, world) in &worlds {
        let countries = world.countries();
        let of_place: std::collections::BTreeMap<_, _> = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive() && p.has_matured())
            .filter_map(|(id, _)| world.society.place_of(id).map(|p| (id, p)))
            .collect();
        let (mut within, mut across) = (0usize, 0usize);
        for (id, here) in &of_place {
            for (other, tie) in world.bonds.of(*id) {
                if !tie.holds() {
                    continue;
                }
                match of_place.get(&other) {
                    Some(there) if there == here => within += 1,
                    Some(_) => across += 1,
                    None => {}
                }
            }
        }
        let moves = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, sim::Happening::PersonMoves { .. }))
            .count();
        println!(
            "  seed {seed:x}: {} countries, {} inhabited quarters; ties {within} within a \
             quarter and {across} across ({:.1}%); {moves} moves ever",
            countries.len(),
            world
                .places
                .ids()
                .filter(|p| world.society.households_in(*p).count() > 0)
                .count(),
            100.0 * across as f32 / (within + across).max(1) as f32
        );
    }

    // ── 3. Is the communication intelligent? ──────────────────────────────────────────────
    //
    // The measurable half of an unanswerable question. `hearsay` is the only channel by which
    // a fact about one person reaches somebody who has never met them, so: how many opinions
    // are held about strangers, and do they agree with the people who *have* met them? An
    // opinion that travels and arrives wrong is gossip; one that does not travel at all is not
    // communication.
    println!("\n── 3. Is the communication intelligent? ──\n");
    for (seed, world) in &worlds {
        let (mut secondhand, mut firsthand) = (0usize, 0usize);
        let (mut agree, mut compared) = (0.0f32, 0usize);
        // What each person is thought of by those who know them well, for comparison.
        let met: std::collections::BTreeMap<_, (f32, usize)> = {
            let mut said: std::collections::BTreeMap<_, (f32, usize)> = Default::default();
            for (id, person) in world.people.iter() {
                if !person.is_alive() {
                    continue;
                }
                for (other, tie) in world.bonds.of(id) {
                    if tie.known > 0.3 {
                        let e = said.entry(other).or_insert((0.0, 0));
                        e.0 += tie.regard;
                        e.1 += 1;
                    }
                }
            }
            said
        };
        for (id, person) in world.people.iter() {
            if !person.is_alive() || !person.has_matured() {
                continue;
            }
            for (other, tie) in world.bonds.of(id) {
                if !tie.holds() {
                    continue;
                }
                // Barely met, but an opinion is held: that opinion came through somebody else.
                if tie.known < 0.12 && tie.regard.abs() > 0.02 {
                    secondhand += 1;
                    if let Some((total, n)) = met.get(&other).filter(|(_, n)| *n >= 3) {
                        let truth = total / *n as f32;
                        agree += 1.0 - (tie.regard - truth).abs().min(2.0) / 2.0;
                        compared += 1;
                    }
                } else if tie.known >= 0.3 {
                    firsthand += 1;
                }
            }
        }
        println!(
            "  seed {seed:x}: {firsthand} opinions from knowing somebody, {secondhand} from \
             hearsay alone ({:.0}% of all held); hearsay agrees with those who know them {:.0}%",
            100.0 * secondhand as f32 / (firsthand + secondhand).max(1) as f32,
            100.0 * agree / compared.max(1) as f32
        );
    }

    // ── 4. Does the world progress? ───────────────────────────────────────────────────────
    //
    // Not "do advances happen" — §29 established that — but whether they compound. A world
    // that works out one new thing per century forever is not progressing, it is idling; the
    // signature of progress is a rate that rises with the population that carries it.
    println!("\n── 4. Does the world progress? ──\n");
    println!(
        "  {:<6} {:>8} {:>9} {:>10} {:>12} {:>16} {:>10}",
        "year", "living", "advances", "technique", "per century", "places with slack", "mean slack"
    );
    for (n, track) in over_time.iter().enumerate() {
        println!("  seed {:x}", seeds[n]);
        let mut last = (0u64, 0usize);
        for (year, advances, technique, living, with_slack, places, mean_slack) in track {
            let per_century = if *year > last.0 {
                (advances - last.1) as f32 * 100.0 / (year - last.0) as f32
            } else {
                0.0
            };
            println!(
                "  {year:<6} {living:>8} {advances:>9} {technique:>10.3} {per_century:>12.1} \
                 {:>16} {mean_slack:>10.4}",
                format_args!("{with_slack} of {places}")
            );
            last = (*year, *advances);
        }
    }
}
