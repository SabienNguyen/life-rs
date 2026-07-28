//! Everything a change to this world can quietly break, in one run.
//!
//! Three mechanisms were built and reverted in one night — §26.11, §27.10, and a household
//! head — and every one of them was caught by the same test, eight minutes into a full suite,
//! after the change had already been committed. Each time the question was the same: *what did
//! that do to the world?* and answering it meant remembering which of six scattered examples
//! and two test modules to run.
//!
//! This is that question, asked once. It is not a test and asserts nothing — the suite is
//! where claims live. It is for the minute after a change, before deciding whether the change
//! is worth measuring properly.
//!
//!     cargo run --release --example vitals
//!
//! Where the world stands, measured rather than remembered — three seeds, 120 founders, 90
//! years, which is what the defaults produce:
//!
//!     living      667
//!     churn         9%   82 of 929 moves went straight back. Over 10% is pathological (§30.4)
//!     biggest    0.55    share of households in one quarter. 1.00 is the collapse (§30.5)
//!     empty      0.33    quarters with nobody in them
//!     spread     0.11    how far apart the inhabited quarters are. §14.4 needs this above 0
//!     short      0.00    the hungriest quarter's shortfall. Should be small; zero at this
//!                        size is expected, since §21's ceiling wants a crowded world
//!     advances     12    things anybody ever worked out (§29)
//!     taken up    108    people a patron ever opened a door for (§25)
//!     trades           farm 318  hew 13  smith 6  cook 48  keep 19 — thin but not empty
//!     assimilation 0.139, and 0.212 for somebody who has moved against 0.066 for somebody
//!                        who has not — §17.2.1's claim, in a running world
//!
//! Seventy-eight seconds, against eight minutes for the suite that would otherwise tell you.
//!
//! **Add a line here before ablating the mechanism it belongs to.** §31.2 switches mechanisms
//! off and compares against this, and an instrument that cannot see what a mechanism claims
//! will report that switching it off changed nothing — which is the same sentence as "the
//! mechanism is inert" and means something entirely different. `advances` and `taken up` are
//! here for exactly that reason, added before §29's and §25's turn came up.

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

const SEEDS: [u128; 3] = [0x11, 0x21, 0x221];

fn main() {
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);
    let founders: usize = std::env::var("FOUNDERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);

    let (mut moves, mut back) = (0usize, 0usize);
    let (mut biggest, mut empty, mut spread, mut short) = (0.0, 0.0, 0.0, 0.0);
    let mut trades = [0usize; 5];
    let mut living = 0;
    let (mut apart, mut counted) = (0.0f32, 0usize);
    let (mut advances, mut taken_up) = (0usize, 0usize);
    let (mut takings, mut countries) = (0usize, 0usize);
    let (mut moved_apart, mut moved_counted) = (0.0f32, 0usize);
    let (mut stayed_apart, mut stayed_counted) = (0.0f32, 0usize);

    for seed in SEEDS {
        let mut world = World::genesis(WorldSeed::from_u128(seed), founders);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.run_for(Duration::from_years(years));
        living += world.living();

        // Churn: households going back where they were two moves ago. The single most
        // sensitive number here — every mechanism reverted so far moved this one first.
        let mut path: std::collections::BTreeMap<u64, Vec<u64>> = Default::default();
        let mut movers: std::collections::BTreeSet<u64> = Default::default();
        for record in world.chronicle.iter() {
            if let sim::Happening::PersonMoves { person, to } = record.kind {
                path.entry(person.to_bits()).or_default().push(to.to_bits());
                movers.insert(person.to_bits());
            }
        }
        for steps in path.values() {
            moves += steps.len();
            back += (2..steps.len()).filter(|i| steps[*i] == steps[i - 2]).count();
        }

        // Where everybody ended up, and whether the quarters still differ.
        let counts: Vec<usize> = world
            .places
            .ids()
            .map(|id| world.society.households_in(id).count())
            .collect();
        let total: usize = counts.iter().sum();
        biggest += *counts.iter().max().unwrap_or(&0) as f32 / total.max(1) as f32;
        empty += counts.iter().filter(|c| **c == 0).count() as f32 / counts.len().max(1) as f32;

        let lived_in: Vec<f32> = world
            .places
            .ids()
            .filter(|id| world.society.households_in(*id).count() > 0)
            .filter_map(|id| world.places.get(id).map(|p| p.env.affluence))
            .collect();
        let mean = lived_in.iter().sum::<f32>() / lived_in.len().max(1) as f32;
        spread += (lived_in.iter().map(|a| (a - mean).powi(2)).sum::<f32>()
            / lived_in.len().max(1) as f32)
            .sqrt();

        // What anybody ever worked out — §29's only output, and a thing no other line here
        // can see. An instrument you ablate against has to be able to see what the mechanism
        // claims, or switching it off looks like it changed nothing for the wrong reason.
        advances += world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, sim::Happening::PersonWorksItOut { .. }))
            .count();
        // What anybody ever took from anybody. Added *before* the mechanism that produces
        // it — §31.2's rule, learned by ablating a famine mechanism in a world with no
        // famine and a discovery mechanism against an instrument that could not see a
        // discovery. An instrument that cannot see a taking will report that conquest
        // changed nothing, which is the same sentence as "it never fires" and means
        // something else entirely.
        // How many countries there are to take from each other. A taking needs two, and
        // §24's peoples take a century or two to split — so a fixture that has not run long
        // enough to have a second country cannot answer anything about conquest, however
        // hungry it is.
        countries += world.countries().len();
        takings += world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, sim::Happening::PlaceTaken { .. }))
            .count();
        // And who was ever taken up, which is §25's largest single fact about a life.
        taken_up += world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, sim::Happening::PersonMentored { .. }))
            .count();

        short += world
            .places
            .iter()
            .filter(|(id, _)| world.society.households_in(*id).count() > 0)
            .map(|(_, p)| p.want)
            .fold(0.0_f32, f32::max);

        for (_, person) in world.people.iter() {
            if person.is_alive() && person.has_matured() {
                trades[person.trade() as usize] += 1;
            }
        }

        // How far anybody's picture of local practice is from local practice, and how far
        // two people standing in the same place are from each other. §17.2.1 claims both are
        // non-zero — that a newcomer is not as steeped as somebody born here. If they come
        // out at nothing then that mechanism is right and inert, which is what happened to
        // the belief on a tie (§17.2.3), and is worth knowing before it is claimed again.
        for (id, person) in world.people.iter() {
            if !person.is_alive() || !person.has_matured() {
                continue;
            }
            let Some(here) = world
                .society
                .place_of(id)
                .and_then(|p| world.places.get(p))
                .map(|p| p.env.norms)
            else {
                continue;
            };
            let gap: f32 = person
                .norms()
                .iter()
                .zip(&here)
                .map(|(mine, theirs)| (mine - theirs).abs())
                .sum::<f32>()
                / person.norms().len() as f32;
            apart += gap;
            counted += 1;
            // And the claim itself, which is narrower than "the number is not zero": that
            // somebody who *moved* carries where they came from. Split the same measurement
            // by whether this person has ever moved house.
            if movers.contains(&id.to_bits()) {
                moved_apart += gap;
                moved_counted += 1;
            } else {
                stayed_apart += gap;
                stayed_counted += 1;
            }
        }
    }

    let n = SEEDS.len() as f32;
    println!("{} seeds, {founders} founders, {years} years\n", SEEDS.len());
    println!("  living     {:>6}   across all three", living);
    println!(
        "  churn      {:>5.0}%   {back} of {moves} moves went straight back",
        100.0 * back as f32 / moves.max(1) as f32
    );
    println!("  biggest    {:>6.2}   share of households in one quarter", biggest / n);
    println!("  empty      {:>6.2}   quarters with nobody in them", empty / n);
    println!("  spread     {:>6.2}   how far apart the inhabited quarters are", spread / n);
    println!("  short      {:>6.2}   the hungriest quarter's shortfall per head", short / n);
    println!("  advances   {advances:>6}   things anybody ever worked out (§29)");
    println!("  taken up   {taken_up:>6}   people a patron ever opened a door for (§25)");
    println!("  takings    {takings:>6}   times anybody took anything by force (§32)");
    println!("  countries  {:>6.1}   how many there are to take from each other", countries as f32 / n);
    println!(
        "\n  trades     {}",
        ["farm", "hew", "smith", "cook", "keep"]
            .iter()
            .enumerate()
            .map(|(at, name)| format!("{name} {}", trades[at]))
            .collect::<Vec<_>>()
            .join("  ")
    );
    println!(
        "\n  assimilation {:>5.3}   how far a person's idea of local practice is from it",
        apart / counted.max(1) as f32
    );
    println!(
        "               {:>5.3}   of somebody who has moved, against {:.3} of somebody who has not",
        moved_apart / moved_counted.max(1) as f32,
        stayed_apart / stayed_counted.max(1) as f32
    );
    println!("\n  (§15's bands need `cargo test -p observer` — they cost six minutes.)");
}
