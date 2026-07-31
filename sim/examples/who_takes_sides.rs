//! Whether anybody in this world is in a position to take a side.
//!
//! Every act in §35 is one person to one person, and every consequence is too: the victim
//! holds a grudge, the actor holds a guilt, and up to three witnesses lower their opinion by
//! six hundredths. Nobody has ever done anything *together*, and nobody has ever turned on
//! somebody because of what was done to a third person. That is the largest remaining gap
//! between this and a society — feuds, factions, ostracism and collective punishment are all
//! the same shape, and none of them are reachable from a vocabulary that is strictly dyadic.
//!
//! The obvious mechanism is **partisanship**: what somebody makes of a wrong should depend on
//! who it was done *to*, not only on how bad it was. Hurting my brother is not the same event
//! as hurting a stranger, and a community where that is true splits along its existing ties
//! when somebody is wronged — which is a faction, arrived at without a faction ever being
//! declared.
//!
//! **This measures whether the world can supply that before any of it is built.** It needs a
//! third person who knows both parties, and it needs that third person to feel differently
//! about them — a C who is fond of both A and B has no side to take. Three times now this
//! project has built a correct mechanism behind a conjunction the world does not supply:
//! §32.2's conquest keyed on adjacent countries in a world with zero adjacent cross-country
//! pairs, §35's killing behind a gate past the end of the world, and §36.6's envy needing an
//! evening that happens five hundredths of one percent of the time. The pattern is always the
//! same and is always cheaper to check than to debug.
//!
//! It also asks whether collective feeling exists **already**. If some people are widely
//! disliked while others are widely liked, the raw material of ostracism is present and only
//! needs a mechanism to act on it. If dislike is scattered evenly, there is nothing to build
//! on and partisanship would be inventing the structure rather than using it.
//!
//!     cargo run --release --example who_takes_sides

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

const SEEDS: [u128; 3] = [0x11, 0x21, 0x221];

/// Warmth past which somebody is a friend rather than an acquaintance, and past its negation
/// an enemy. Kept here rather than read from `bonds` so this says what it measured.
const FOND: f32 = 0.25;

fn main() {
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);

    // Triangles: for a tie that holds between A and B, how many third parties know them both.
    let (mut pairs, mut with_a_third) = (0usize, 0usize);
    let mut thirds_each: Vec<usize> = Vec::new();
    // And of those third parties, whether they have a side to take.
    let (mut sided, mut fond_of_both, mut cool_on_both) = (0usize, 0usize, 0usize);
    // Standing: how many people hold negative warmth toward each person, against how many
    // hold positive. The question is whether dislike concentrates on somebody or scatters.
    let mut disliked_by: Vec<(usize, usize, String)> = Vec::new();

    for seed in SEEDS {
        let mut world = World::genesis(WorldSeed::from_u128(seed), 120);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.run_for(Duration::from_years(years));

        let living: Vec<_> = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive() && p.has_matured())
            .map(|(id, p)| (id, p.name.clone()))
            .collect();

        // Who each person knows, once, so the triad walk is a set intersection rather than a
        // scan of the world per pair.
        let known: std::collections::BTreeMap<_, std::collections::BTreeSet<_>> = living
            .iter()
            .map(|(id, _)| {
                (
                    *id,
                    world
                        .bonds
                        .of(*id)
                        .filter(|(_, tie)| tie.holds())
                        .map(|(other, _)| other)
                        .collect(),
                )
            })
            .collect();

        for (a, _) in &living {
            let Some(a_knows) = known.get(a) else { continue };
            for b in a_knows {
                // Once per unordered pair. §39 measured this graph to be undirected in
                // everything but its type, so counting both ways would double everything and
                // say nothing.
                if b <= a {
                    continue;
                }
                let Some(b_knows) = known.get(b) else { continue };
                pairs += 1;
                let thirds: Vec<_> = a_knows.intersection(b_knows).copied().collect();
                thirds_each.push(thirds.len());
                if !thirds.is_empty() {
                    with_a_third += 1;
                }
                for c in thirds {
                    let to_a = world.bonds.tie(c, *a).warmth;
                    let to_b = world.bonds.tie(c, *b).warmth;
                    match (to_a > FOND, to_b > FOND) {
                        (true, true) => fond_of_both += 1,
                        (false, false) => cool_on_both += 1,
                        // Fond of exactly one of them — somebody with a side.
                        _ => sided += 1,
                    }
                }
            }
        }

        for (id, name) in &living {
            let (mut against, mut fory) = (0usize, 0usize);
            for (_, tie) in world.bonds.of(*id) {
                if !tie.holds() {
                    continue;
                }
                // Held *about* them is what a reputation is, but §39 found the graph
                // effectively undirected, so what they hold is the same reading and is the one
                // that is a lookup rather than a scan.
                if tie.warmth < -0.05 {
                    against += 1;
                } else if tie.warmth > FOND {
                    fory += 1;
                }
            }
            disliked_by.push((against, fory, name.clone()));
        }
    }

    thirds_each.sort_unstable();
    let at = |p: f32| thirds_each[((thirds_each.len() as f32 - 1.0) * p) as usize];
    let triads: usize = thirds_each.iter().sum();

    println!("{} seeds, {years} years\n", SEEDS.len());
    println!("  Can anybody take a side?\n");
    println!("  pairs who know each other        {pairs:>7}");
    println!(
        "  with at least one mutual friend  {with_a_third:>7}   ({:.1}% — the precondition)",
        100.0 * with_a_third as f32 / pairs.max(1) as f32
    );
    println!(
        "  third parties per pair           median {}, p90 {}, most {}",
        at(0.5),
        at(0.9),
        thirds_each.last().copied().unwrap_or(0)
    );
    println!("  (holder, subject, onlooker) triples  {triads:>7}\n");

    let share = |n: usize| 100.0 * n as f32 / triads.max(1) as f32;
    println!("  of those onlookers:");
    println!(
        "    fond of exactly one           {sided:>7}   ({:.1}%)  <- has a side to take",
        share(sided)
    );
    println!(
        "    fond of both                  {fond_of_both:>7}   ({:.1}%)  <- torn",
        share(fond_of_both)
    );
    println!(
        "    fond of neither               {cool_on_both:>7}   ({:.1}%)  <- indifferent",
        share(cool_on_both)
    );

    // And whether feeling concentrates. A society in which everybody is disliked by two
    // people has no outcasts; one in which a few are disliked by twenty has them already, and
    // ostracism would be a mechanism reading a structure rather than inventing one.
    disliked_by.sort_by_key(|(against, _, _)| std::cmp::Reverse(*against));
    let any = disliked_by.iter().filter(|(a, _, _)| *a > 0).count();
    // Is `regard` alive at all? `vitals` reports the spread of it about a person as 0.001,
    // which has two completely different explanations: everybody agrees because `hearsay` has
    // ground opinion flat, or everybody agrees because the number was never written and they
    // are all sitting at the zero it was born with. Those want opposite responses and the
    // difference is one count.
    println!("\n  Is `regard` alive?\n");
    let mut live = [(0usize, 0usize); 2];
    let mut sums = [0.0f64; 2];
    for seed in SEEDS {
        let mut world = World::genesis(WorldSeed::from_u128(seed), 120);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.run_for(Duration::from_years(years));
        for (who, person) in world.people.iter() {
            if !person.is_alive() {
                continue;
            }
            for (_, tie) in world.bonds.of(who) {
                if !tie.holds() {
                    continue;
                }
                for (at, value) in [(0usize, tie.regard), (1usize, tie.warmth)] {
                    live[at].1 += 1;
                    sums[at] += value.abs() as f64;
                    if value.abs() > 0.01 {
                        live[at].0 += 1;
                    }
                }
            }
        }
    }
    for (at, name) in [(0usize, "regard"), (1usize, "warmth")] {
        let (moved, all) = live[at];
        println!(
            "  {name:<7} moved off zero on {moved:>7} of {all:>7} live ties  ({:>5.1}%),  mean |value| {:.4}",
            100.0 * moved as f32 / all.max(1) as f32,
            sums[at] / all.max(1) as f64
        );
    }

    println!("\n  Is anybody an outcast already?\n");
    println!(
        "  adults disliked by anybody at all  {any:>5} of {}   ({:.1}%)",
        disliked_by.len(),
        100.0 * any as f32 / disliked_by.len().max(1) as f32
    );
    println!("  the most disliked people in three worlds:");
    for (against, fory, name) in disliked_by.iter().take(8) {
        println!("    {name:<24} disliked by {against:>3}, liked by {fory:>3}");
    }
}
