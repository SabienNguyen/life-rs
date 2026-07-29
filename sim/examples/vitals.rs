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
//! Where the world stands, measured rather than remembered — **twelve** seeds (`SEEDS=12`),
//! 120 founders, 90 years:
//!
//!     living     2960
//!     churn        11%   585 of 5380 moves went straight back. Over 10% is pathological (§30.4)
//!     biggest    0.54    share of households in one quarter. 1.00 is the collapse (§30.5)
//!     empty      0.35    quarters with nobody in them
//!     spread     0.12    how far apart the inhabited quarters are. §14.4 needs this above 0
//!     short      0.00    the hungriest quarter's shortfall. Should be small; near zero at
//!                        this size is expected, since §21's ceiling wants a crowded world
//!     advances     66    things anybody ever worked out (§29)
//!     taken up    489    people a patron ever opened a door for (§25)
//!     acts             what people did to each other on purpose (§35)
//!     withheld         times somebody turned away from a neighbour visibly worse off, in a
//!                        place whose ways say you do not
//!     witnessed        times somebody who was not part of it saw (§40)
//!     envy aims        robberies per thousand evenings with the one person somebody measures
//!                        themselves against, against the rate with everybody else — and how
//!                        many of those evenings the envy was strong enough to say anything on.
//!                        Two rates and a count, because a *tally* of robberies cannot tell a
//!                        world where envy aims from one where it only agitates (§36.6)
//!     wants            what their lives so far add up to wanting (§36)
//!     killed           deaths by another person's hand, counted off the death records rather
//!                        than off the act tally, so the two can disagree and be seen to
//!
//! **And then the spread between worlds, for the four numbers that need it.** That block is
//! the most important thing this prints, and the numbers in it are deliberately **not** quoted
//! here. One world in twelve can put every household in a single quarter while another has
//! quarters that do not differ at all, with nothing changed — so a mechanism that moves
//! `biggest` by five hundredths has moved it by a fraction of one standard deviation, and three
//! sections of the design document rest on differences that size. **An instrument that does not
//! report its own precision is not an instrument, it is a number** (§40.3).
//!
//! And the precision has to come from the same run as the effect, which is why there is nothing
//! to quote. Three builds within noise of each other gave `empty` a standard deviation of 0.094,
//! then 0.149, then 0.185, and `biggest` 0.122, 0.100, 0.145. Twelve worlds pin an sd to about a
//! fifth of itself at best, so a noise floor carried over from a previous run is a threshold
//! that may be half or twice what it says (§40.3.2). Read the block this prints; do not
//! remember it.
//!
//! `ACTS=0` switches §35's vocabulary off, `WITNESS=0` §40's, `CHANGE=0` §41's and `ENVY=0`
//! §36.6's, on the same instrument, for the comparison.
//!
//! `ENVY=0` is the one worth studying, because it is the only switch here that has to leave a
//! **reading** in place while removing a **mechanism**. Who somebody envies is also the
//! denominator of the rate that judges envy, so a switch that took the reading away with the
//! mechanism would compare two different denominators and report whatever it liked. §36.6 has
//! the account; the short version is that the aim rate came out fourteen times higher at the
//! envied, came out *identically* fourteen times higher with the mechanism switched off, and
//! stayed that way through two wrong diagnoses before a test found the arithmetic error that
//! two cancelling bugs had been hiding.
//!
//! `ONE_DREAM=1` is not an ablation of a mechanism but of a *representation*: it restores the
//! winner-take-all channel §36.6 replaced, so that what the replacement cost stays measurable
//! instead of remembered. It moves what people do to each other well outside the noise floor and
//! moves the shape of the world not at all, which is worth seeing once.
//!
//! Under a minute at three seeds, five at twelve, against eight minutes for the suite that
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
    // `ACTS=0` switches §35's vocabulary off. The ablation lives in the instrument rather
    // than in a script that edits a constant and rebuilds, because two ablations in this
    // project have left the working tree holding an edited constant after the container
    // running them restarted — and an ablation nobody can run without editing the source is
    // an ablation nobody runs.
    let acts = std::env::var("ACTS").map(|v| v != "0").unwrap_or(true);
    // And `WITNESS=0` for §40, separately: whether people do things to each other and whether
    // anybody standing there notices are two claims, and §31.2's table wants a row for each.
    let seen = std::env::var("WITNESS").map(|v| v != "0").unwrap_or(true);
    // And `CHANGE=0` for §41 — whether a life changes who somebody is.
    let changing = std::env::var("CHANGE").map(|v| v != "0").unwrap_or(true);
    // And `ENVY=0` for §36.6. The reading of who somebody envies survives the switch even
    // though the mechanism does not, so that the `envy aims` line below is measured over the
    // same evenings in both worlds. Without that it would compare two different denominators
    // and report whatever it liked.
    let envying = std::env::var("ENVY").map(|v| v != "0").unwrap_or(true);
    // And `ONE_DREAM=1` puts back the winner-take-all channel §36.6 replaced, so that what the
    // replacement cost stays a thing anybody can re-measure.
    let one_dream = std::env::var("ONE_DREAM").map(|v| v == "1").unwrap_or(false);

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
    // How many acts anybody who was not part of them ever saw (§40). Counted *before* the
    // mechanism, which is §31.2's rule and the one this project keeps having to relearn: an
    // instrument that cannot see what a mechanism claims reports an ablation of it as having
    // changed nothing, which is the same sentence as "it never fires" and means something
    // else entirely.
    let mut witnessed = 0usize;
    // Whether envy aims (§36.6). Not a count but two rates: robberies per thousand evenings
    // spent with the one person somebody measures themselves against, against robberies per
    // thousand evenings spent with anybody else. A count of robberies cannot tell those
    // worlds apart, which is the §31.2 mistake in its purest form — the mechanism does not
    // claim more robbery, it claims robbery *lands somewhere*. And beside them the count of
    // evenings the envy was strong enough to say anything at all, which is the number that
    // turned out to settle it.
    let (mut occasions, mut met_envied, mut robbed_envied) = (0u64, 0u64, 0usize);
    let mut told_envied = 0u64;
    // How far a life has moved people from the temperaments they grew up with (§41). Here
    // before the mechanism, for §31.2's reason.
    let (mut weathered, mut weathered_of) = (0.0f32, 0usize);
    let mut weathered_most = 0.0f32;
    // Per world as well as pooled, so the **noise floor of these numbers is computable from
    // the same run that reports them**. This is the question that has come up over and over:
    // a change moves `spread` by four hundredths and there is no way to know whether that is
    // the change or the fact that any change at all reshuffles which quarter fills up.
    // Printing the spread between worlds answers it without a second run.
    let mut per_seed: Vec<(u128, f32, f32, f32)> = Vec::new();
    let mut churn_each: Vec<f32> = Vec::new();
    // What people are after (§36). Here for §31.2's reason and no other: dreams act only
    // through acts, so an ablation of the vocabulary switches them off too — and an
    // instrument that could not see them would report that as having cost nothing.
    let mut dreamt = [0usize; person::dreams::Dream::COUNT];
    let mut dreamers = 0usize;

    for seed in seeds.iter().copied() {
        let mut world = World::genesis(WorldSeed::from_u128(seed), founders);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.acts_are_possible = acts;
        world.witnesses_notice = seen;
        world.people_change = changing;
        world.people_envy = envying;
        world.only_the_strongest_dream = one_dream;
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
        let (mut moves_here, mut back_here) = (0usize, 0usize);
        for steps in path.values() {
            moves_here += steps.len();
            back_here += (2..steps.len()).filter(|i| steps[*i] == steps[i - 2]).count();
        }
        moves += moves_here;
        back += back_here;
        churn_each.push(back_here as f32 / moves_here.max(1) as f32);

        // Where everybody ended up, and whether the quarters still differ.
        let counts: Vec<usize> = world
            .places
            .ids()
            .map(|id| world.society.households_in(id).count())
            .collect();
        let total: usize = counts.iter().sum();
        let biggest_here = *counts.iter().max().unwrap_or(&0) as f32 / total.max(1) as f32;
        let empty_here = counts.iter().filter(|c| **c == 0).count() as f32 / counts.len().max(1) as f32;
        biggest += biggest_here;
        empty += empty_here;
        per_seed.push((seed, biggest_here, empty_here, 0.0));

        let lived_in: Vec<f32> = world
            .places
            .ids()
            .filter(|id| world.society.households_in(*id).count() > 0)
            .filter_map(|id| world.places.get(id).map(|p| p.env.affluence))
            .collect();
        let mean = lived_in.iter().sum::<f32>() / lived_in.len().max(1) as f32;
        let spread_here = (lived_in.iter().map(|a| (a - mean).powi(2)).sum::<f32>()
            / lived_in.len().max(1) as f32)
            .sqrt();
        spread += spread_here;
        if let Some(last) = per_seed.last_mut() {
            last.3 = spread_here;
        }

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
        witnessed += world.witnessed as usize;
        occasions += world.occasions;
        met_envied += world.met_the_envied;
        told_envied += world.told_the_envied;
        robbed_envied += world.robbed_the_envied as usize;
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
            // How far this life has carried them from who they finished growing up as.
            let moved = {
                let w = person.weathering();
                (w.openness.abs() + w.extraversion.abs() + w.agreeableness.abs()
                    + w.neuroticism.abs()) / 4.0
            };
            weathered += moved;
            weathered_of += 1;
            weathered_most = weathered_most.max(moved);
            if let Some((dream, _)) = world
                .what_they_have_come_to(id)
                .and_then(|come_to| person::dreams::of(person, &come_to, world.now()))
            {
                dreamt[dream as usize] += 1;
                dreamers += 1;
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
    println!("  witnessed  {witnessed:>6}   times somebody who was not part of it saw (§40)");
    {
        let per_thousand =
            |robs: usize, evenings: u64| 1000.0 * robs as f32 / evenings.max(1) as f32;
        println!(
            "  envy aims  {:>6.2}   robberies per thousand evenings with the one they envy, \
             against {:.2} with anybody else",
            per_thousand(robbed_envied, met_envied),
            per_thousand(
                acted[person::acts::Toward::Rob as usize] - robbed_envied,
                occasions - met_envied
            ),
        );
        println!(
            "             {told_envied:>6}   of those {met_envied} evenings had the envy strong enough to say \
             anything, of {occasions} in all (§36.6)"
        );
    }
    println!(
        "  weathered  {:>6.3}   how far a life moves a temperament, per trait; worst {:.2} (§41)",
        weathered / weathered_of.max(1) as f32,
        weathered_most
    );
    println!(
        "\n  wants      {}",
        person::dreams::Dream::ALL
            .iter()
            .map(|d| format!("{} {}", d.label(), dreamt[*d as usize]))
            .collect::<Vec<_>>()
            .join("  ")
    );
    println!("             {dreamers:>6}   adults after something in particular (§36)");
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
    // And how much of the above is the world rather than the measurement.
    if per_seed.len() > 1 {
        let sigma = |pick: fn(&(u128, f32, f32, f32)) -> f32| {
            let n = per_seed.len() as f32;
            let mean = per_seed.iter().map(pick).sum::<f32>() / n;
            let var = per_seed.iter().map(|row| (pick(row) - mean).powi(2)).sum::<f32>() / n;
            (var.sqrt(), var.sqrt() / n.sqrt())
        };
        println!("\n  how much of that is the measurement — spread between worlds, and the");
        println!("  standard error on the mean above:");
        for (name, pick) in [
            ("biggest", (|r: &(u128, f32, f32, f32)| r.1) as fn(&(u128, f32, f32, f32)) -> f32),
            ("empty", |r: &(u128, f32, f32, f32)| r.2),
            ("spread", |r: &(u128, f32, f32, f32)| r.3),
        ] {
            let (sd, se) = sigma(pick);
            println!("    {name:<9} sd {sd:.3}  se {se:.3}   worlds: {}",
                per_seed.iter().map(|r| format!("{:.2}", pick(r))).collect::<Vec<_>>().join(" "));
        }
        // Churn separately, because it is a ratio per world rather than a mean of one.
        let n = churn_each.len() as f32;
        let mean = churn_each.iter().sum::<f32>() / n;
        let sd = (churn_each.iter().map(|c| (c - mean).powi(2)).sum::<f32>() / n).sqrt();
        println!("    {:<9} sd {sd:.3}  se {:.3}   worlds: {}", "churn", sd / n.sqrt(),
            churn_each.iter().map(|c| format!("{:.2}", c)).collect::<Vec<_>>().join(" "));
    }

    println!("\n  (§15's bands need `cargo test -p observer` — they cost six minutes.)");
}
