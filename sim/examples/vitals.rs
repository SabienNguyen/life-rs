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
//! Where the world stands, measured rather than remembered — **eight** seeds (`SEEDS=8`), 120
//! founders, 90 years:
//!
//!     living     2019
//!     churn         8%   318 of 3792 moves went straight back. Over 10% is pathological (§30.4)
//!     biggest    0.64    share of households in one quarter. 1.00 is the collapse (§30.5)
//!     empty      0.40    quarters with nobody in them
//!     spread     0.13    how far apart the inhabited quarters are. §14.4 needs this above 0
//!     short      0.02    the hungriest quarter's shortfall. Should be small; near zero at
//!                        this size is expected, since §21's ceiling wants a crowded world
//!     advances     37    things anybody ever worked out (§29)
//!     taken up    346    people a patron ever opened a door for (§25)
//!     trades           farm 950  hew 39  smith 33  cook 132  keep 37 — thin but not empty
//!     acts             gave to 779  taught 153  shunned 271  robbed 97  killed 6 — what
//!                        people did to each other on purpose (§35)
//!     withheld   4762    times somebody turned away from a neighbour visibly worse off, in a
//!                        place whose ways say you do not
//!     killed        6    deaths by another person's hand, counted off the death records
//!                        rather than off the act tally, so the two can disagree and be seen to
//!     assimilation 0.117, and 0.132 for somebody who has moved against 0.066 for somebody
//!                        who has not — §17.2.1's claim, in a running world
//!
//! Under a minute at three seeds, three at eight, against eight minutes for the suite that
//! would otherwise tell you.
//!
//! **Add a line here before ablating the mechanism it belongs to.** §31.2 switches mechanisms
//! off and compares against this, and an instrument that cannot see what a mechanism claims
//! will report that switching it off changed nothing — which is the same sentence as "the
//! mechanism is inert" and means something entirely different. `advances` and `taken up` are
//! here for exactly that reason, added before §29's and §25's turn came up.

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

/// The worlds this asks about.
///
/// Three by default because that is what a minute buys, and `SEEDS=n` for more — which is
/// not a convenience. Two of the numbers below, `biggest` and `empty`, swing by twenty
/// points at three seeds on a change that added nineteen robberies to a world of six
/// hundred people. They are not measuring the change; they are measuring the fact that
/// *any* change reshuffles which quarter happens to fill up. A mechanism cannot be judged
/// against a statistic whose noise floor is larger than any effect it could have, and the
/// only cure is more worlds.
const ALL_SEEDS: [u128; 12] = [
    0x11, 0x21, 0x221, 0x31, 0x41, 0x5ee, 0x77, 0x8a, 0x91, 0xa3, 0xbb, 0xc7,
];

fn main() {
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);
    let founders: usize = std::env::var("FOUNDERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);
    let how_many: usize = std::env::var("SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3)
        .clamp(1, ALL_SEEDS.len());
    let seeds = &ALL_SEEDS[..how_many];

    let (mut moves, mut back) = (0usize, 0usize);
    let (mut biggest, mut empty, mut spread, mut short) = (0.0, 0.0, 0.0, 0.0);
    let mut trades = [0usize; 5];
    let mut living = 0;
    let (mut apart, mut counted) = (0.0f32, 0usize);
    let (mut advances, mut taken_up) = (0usize, 0usize);
    let (mut takings, mut countries) = (0usize, 0usize);
    let (mut moved_apart, mut moved_counted) = (0.0f32, 0usize);
    let (mut stayed_apart, mut stayed_counted) = (0.0f32, 0usize);
    let mut acted = [0usize; person::acts::Toward::COUNT];
    let (mut withheld, mut killed) = (0usize, 0usize);

    for seed in seeds.iter().copied() {
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

        // What people did to each other on purpose (§35). Read off the world's own tally
        // rather than the chronicle, because four of the five are recorded at `Notable` and
        // every run here asks for `Pivotal` only — an instrument that could not see them at
        // the detail everybody actually uses would report an ablation of the whole
        // vocabulary as having changed nothing.
        for (at, count) in world.acted.iter().enumerate() {
            acted[at] += *count as usize;
        }
        withheld += world.withheld as usize;
        // Killings are counted a second way, off the death records, because they are the one
        // act whose consequence is somebody being gone — and a tally that says five murders
        // in a world where nobody died of violence is a bug in one of the two.
        killed += world
            .chronicle
            .iter()
            .filter(|r| {
                matches!(
                    r.kind,
                    sim::Happening::PersonDies {
                        cause: person::Cause::Violence,
                        ..
                    }
                )
            })
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

    let n = seeds.len() as f32;
    println!("{} seeds, {founders} founders, {years} years\n", seeds.len());
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
        "\n  acts       {}",
        person::acts::Toward::ALL
            .iter()
            .map(|act| format!("{} {}", act.label(), acted[*act as usize]))
            .collect::<Vec<_>>()
            .join("  ")
    );
    println!("  withheld   {withheld:>6}   times somebody turned away where that is not done");
    println!("  killed     {killed:>6}   deaths by another person's hand");
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
