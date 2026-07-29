//! How pairings actually go, before anything is done about the ones that go badly.
//!
//! §33 counted what people are to each other and found that **nobody separates**: 647 pairings
//! in one world and the only exit is a death. Whether that is a fact about this society or a
//! missing mechanism turns on one measurement — *do any of them sour?* If every partnership
//! in every world sits at warm-and-getting-warmer, then a separation mechanism would be a
//! rule with nothing to fire on, which is §32's whole lesson twice over: conquest keyed on
//! adjacent countries that could not exist, and famine relief measured in a world with no
//! famine.
//!
//! So this asks the question the mechanism would need answered, and asks it first.
//!
//!     cargo run --release --example how_it_goes

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

const SEEDS: [u128; 3] = [0x11, 0x21, 0x221];

fn main() {
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);

    let (mut pairs, mut both_warm, mut one_cold, mut both_cold) = (0usize, 0usize, 0usize, 0usize);
    let mut warmths: Vec<f32> = Vec::new();
    let mut worst: Vec<(f32, String, String, f64)> = Vec::new();
    // And how long they had been together, because a pairing that sours in year two is a
    // different claim from one that sours after thirty.
    let (mut sour_years, mut sour_count) = (0.0f64, 0usize);

    for seed in SEEDS {
        let mut world = World::genesis(WorldSeed::from_u128(seed), 140);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.run_for(Duration::from_years(years));
        let now = world.now();

        // When each pairing happened, so "how long" is answerable.
        let mut paired_at: std::collections::BTreeMap<(person::PersonId, person::PersonId), _> =
            Default::default();
        for record in world.chronicle.iter() {
            if let sim::Happening::PersonPairs { person, with } = record.kind {
                let key = if person < with {
                    (person, with)
                } else {
                    (with, person)
                };
                paired_at.insert(key, record.at);
            }
        }

        let mut seen: std::collections::BTreeSet<(person::PersonId, person::PersonId)> =
            Default::default();
        for (id, person) in world.people.iter() {
            if !person.is_alive() {
                continue;
            }
            let Some(other) = world.society.partner_of(id) else {
                continue;
            };
            if !world.people.get(other).is_some_and(|p| p.is_alive()) {
                continue;
            }
            let key = if id < other { (id, other) } else { (other, id) };
            if !seen.insert(key) {
                continue;
            }
            pairs += 1;
            let mine = world.bonds.tie(id, other).warmth;
            let theirs = world.bonds.tie(other, id).warmth;
            warmths.push((mine + theirs) / 2.0);
            let together = paired_at
                .get(&key)
                .map(|at| now.since(*at).as_years())
                .unwrap_or(0.0);
            if mine > 0.0 && theirs > 0.0 {
                both_warm += 1;
            } else if mine < 0.0 && theirs < 0.0 {
                both_cold += 1;
                sour_years += together;
                sour_count += 1;
                let name = |who| {
                    world
                        .people
                        .get(who)
                        .map(|p: &person::Person| p.name.clone())
                        .unwrap_or_default()
                };
                worst.push(((mine + theirs) / 2.0, name(id), name(other), together));
            } else {
                one_cold += 1;
                sour_years += together;
                sour_count += 1;
            }
        }
    }

    warmths.sort_by(f32::total_cmp);
    let at = |q: f32| {
        warmths
            .get(((warmths.len() as f32 - 1.0) * q) as usize)
            .copied()
            .unwrap_or(f32::NAN)
    };

    println!("{} seeds, {years} years\n", SEEDS.len());
    println!("  living pairings           {pairs:>6}");
    println!(
        "  both still fond           {both_warm:>6}   {:>5.1}%",
        100.0 * both_warm as f32 / pairs.max(1) as f32
    );
    println!(
        "  one of the two has gone   {one_cold:>6}   {:>5.1}%",
        100.0 * one_cold as f32 / pairs.max(1) as f32
    );
    println!(
        "  both have                 {both_cold:>6}   {:>5.1}%",
        100.0 * both_cold as f32 / pairs.max(1) as f32
    );
    println!(
        "\n  warmth between partners: worst {:.2}, tenth {:.2}, middle {:.2}, best {:.2}",
        at(0.0),
        at(0.1),
        at(0.5),
        at(1.0)
    );
    if sour_count > 0 {
        println!(
            "  and the ones that have gone had been together {:.0} years on average",
            sour_years / sour_count as f64
        );
    }

    worst.sort_by(|a, b| a.0.total_cmp(&b.0));
    if worst.is_empty() {
        println!(
            "\n  Not one pairing in any of these worlds has gone bad on both sides. A rule for\n  \
             ending them would be a rule with nothing to fire on."
        );
    } else {
        println!("\n  the worst of them:");
        for (warmth, one, two, together) in worst.iter().take(8) {
            println!("    {one:<22} and {two:<22} {warmth:>6.2}, {together:.0} years");
        }
    }

    // And the thing underneath all of it: how well two people in this world go together at
    // all. `meet_repeatedly` drives warmth toward `suits * 2 - 1`, so a mean `suits` below a
    // half means the *average pair of people here mildly dislike each other* — which nobody
    // would have chosen, and which would make a third of pairings going cold a fact about a
    // normalising constant rather than about anybody's temperament.
    let mut world = World::genesis(WorldSeed::from_u128(SEEDS[0]), 140);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);
    world.run_for(Duration::from_years(years));
    let adults: Vec<&person::Person> = world
        .people
        .iter()
        .filter(|(_, p)| p.is_alive() && p.has_matured())
        .map(|(_, p)| p)
        .collect();
    let mut suited: Vec<f32> = Vec::new();
    let mut compat: Vec<f32> = Vec::new();
    for (at, one) in adults.iter().enumerate() {
        for two in adults.iter().skip(at + 1).take(40) {
            suited.push(bonds::suits(&one.personality, &two.personality));
            compat.push(one.compatibility(two));
        }
    }
    let mean = |of: &[f32]| of.iter().sum::<f32>() / of.len().max(1) as f32;
    println!(
        "  two people at random: suits {:.3} (warmth aims at {:+.3}), compatibility {:.3}",
        mean(&suited),
        mean(&suited) * 2.0 - 1.0,
        mean(&compat)
    );
    println!("  over {} pairs.", suited.len());
    println!(
        "  `seek_partner` picks the best of eight on *compatibility*; a tie then warms toward\n  \
         `suits * 2 - 1`. Two functions of the same five numbers, a Euclidean distance over\n  \
         six and a Manhattan one over ten — so the choosing maximises one quantity and the\n  \
         living runs on another. Unifying them was tried (§38): it moved the median pairing's\n  \
         warmth from 0.04 to 0.05 — which is how little the best of eight is ever buying — and\n  \
         it is not in the world, because §15's shared-environment band failed on the trajectory\n  \
         shift and that band is measured over three worlds."
    );
}
