//! Nobody in this world has ever killed anybody. This asks whether that is a finding.
//!
//! `Toward::Kill` needs both halves of a sentence — they hate them, *and* they have nothing
//! to lose — and a conjunction that never fires looks exactly like a mechanism that is
//! broken. The two are distinguished by measuring the halves separately: if hatred is common
//! and ruin is common and no one person is ever both, that is a fact about this society. If
//! one of the halves is *never* true of anybody, the gate is set past the end of the world
//! and the conjunction was never the reason.
//!
//! This is the same lesson as §32.2, where conquest was keyed on adjacent countries and there
//! turned out to be **zero adjacent cross-country pairs in any world at any size** — a
//! theorem rather than a rare case, and one that no amount of tuning the raid threshold would
//! have found.
//!
//!     cargo run --release --example who_could_kill

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

/// The two gates, kept here rather than read from `person::acts` so that this says what it
/// measured even after somebody moves them.
const HATRED: f32 = 0.45;
const DESPERATE: f32 = 0.5;

fn main() {
    let seed = std::env::var("SEED")
        .ok()
        .and_then(|v| u128::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x11);
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);

    let mut world = World::genesis(WorldSeed::from_u128(seed), 120);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);
    world.run_for(Duration::from_years(years));
    let now = world.now();
    let whole_life = life::Mortality::HUMAN.median_lifespan();

    // How far gone each living adult is, by the same three anchors `nothing_to_lose` uses.
    let mut spent: std::collections::BTreeMap<person::PersonId, f32> = Default::default();
    for (id, person) in world.people.iter() {
        if !person.is_alive() || !person.has_matured() {
            continue;
        }
        let dependents = world
            .society
            .children_of(id)
            .iter()
            .filter(|c| {
                world
                    .people
                    .get(**c)
                    .is_some_and(|p| p.is_alive() && p.stage(now).is_dependent())
            })
            .count();
        let ahead = ((whole_life - person.age(now).years()) / whole_life).clamp(0.0, 1.0) as f32;
        let held_by = [
            person.means() / (person.means() + 0.5),
            if dependents > 0 { 0.85 } else { 0.0 },
            ahead * person.health().vitality,
        ]
        .into_iter()
        .fold(0.0_f32, f32::max);
        spent.insert(id, (1.0 - held_by).clamp(0.0, 1.0));
    }

    // The appetite itself, for every tie in the world, built from exactly what `act_toward`
    // hands the scorer. Measuring the gates alone was not enough: they both fire, and the act
    // still never happened, so the question moved from "can anybody" to "how much do they
    // want to" — and only the number the code actually computes can answer that.
    let mut appetites: Vec<(f32, String, String)> = Vec::new();
    for (holder, person) in world.people.iter() {
        if !person.is_alive() || !person.has_matured() {
            continue;
        }
        let Some(place) = world.society.place_of(holder) else {
            continue;
        };
        let Some(shortfall) = world.places.get(place).map(|p| p.want) else {
            continue;
        };
        let dependents = world
            .society
            .children_of(holder)
            .iter()
            .filter(|c| {
                world
                    .people
                    .get(**c)
                    .is_some_and(|p| p.is_alive() && p.stage(now).is_dependent())
            })
            .count();
        let ahead = ((whole_life - person.age(now).years()) / whole_life).clamp(0.0, 1.0) as f32;
        let come_to = world.what_they_have_come_to(holder);
        let actor = person::acts::Actor {
            values: &person.values,
            personality: &person.personality,
            held: person.held(),
            means: person.means(),
            want: shortfall,
            dependents,
            health: person.health().vitality,
            life_ahead: ahead,
            has_a_trade: person.has_matured(),
            own_ways: person::acts::what_is_expected(person.norms()),
            envies: come_to.as_ref().and_then(|it| it.envied).map(|envy| envy.of),
            dreams: come_to
                .as_ref()
                .map(|come_to| person::dreams::longings(person, come_to, now))
                .unwrap_or_default(),
        };
        for (about, tie) in world.bonds.of(holder) {
            let Some(them) = world.people.get(about).filter(|p| p.is_alive()) else {
                continue;
            };
            let subject = person::acts::Subject {
                who: about,
                warmth: tie.warmth,
                regard: tie.regard,
                debt: tie.debt,
                known: tie.known,
                means: them.means(),
                want: shortfall,
                age_years: them.age(now).years(),
                matured: them.has_matured(),
            };
            let want = person::acts::weigh(&actor, &subject, now)
                [person::acts::Toward::Kill as usize];
            if want > 0.0 {
                appetites.push((want, person.name.clone(), them.name.clone()));
            }
        }
    }
    appetites.sort_by(|a, b| b.0.total_cmp(&a.0));

    // And how sour each tie has gone.
    let (mut ties, mut hating, mut ruined_holders, mut both) = (0usize, 0usize, 0usize, 0usize);
    let (mut worst_warmth, mut worst_spent) = (0.0_f32, 0.0_f32);
    let mut candidates: Vec<(f32, f32, String, String)> = Vec::new();
    for (holder, _) in world.people.iter() {
        let Some(gone) = spent.get(&holder).copied() else {
            continue;
        };
        worst_spent = worst_spent.max(gone);
        if gone > DESPERATE {
            ruined_holders += 1;
        }
        for (about, tie) in world.bonds.of(holder) {
            if !tie.holds() || tie.known <= 0.2 {
                continue;
            }
            ties += 1;
            let hate = (-tie.warmth).max(0.0);
            worst_warmth = worst_warmth.max(hate);
            if hate > HATRED {
                hating += 1;
                if gone > DESPERATE {
                    both += 1;
                    let name = |id| {
                        world
                            .people
                            .get(id)
                            .map(|p| p.name.clone())
                            .unwrap_or_default()
                    };
                    candidates.push((hate, gone, name(holder), name(about)));
                }
            }
        }
    }

    println!("seed {seed:x}, {years} years, {} living\n", world.living());
    println!("  adults weighed          {:>6}", spent.len());
    println!("  ties known well enough  {ties:>6}");
    println!(
        "  of them, hate > {HATRED:.2}    {hating:>6}   ({:.2}% of ties)",
        100.0 * hating as f32 / ties.max(1) as f32
    );
    println!(
        "  adults with nothing to lose (> {DESPERATE:.2})  {ruined_holders:>4}   ({:.1}% of adults)",
        100.0 * ruined_holders as f32 / spent.len().max(1) as f32
    );
    println!("  ties that are both      {both:>6}");
    println!("\n  the sourest tie anywhere    {worst_warmth:>5.2}  (hate, 0 to 1)");
    println!("  the most spent anybody is   {worst_spent:>5.2}");

    println!(
        "\n  ties with any appetite at all   {:>5}",
        appetites.len()
    );
    println!(
        "  the strongest anybody wants it  {:>5.3}   (the bar is 0.25)",
        appetites.first().map(|a| a.0).unwrap_or(0.0)
    );
    for (want, who, about) in appetites.iter().take(6) {
        println!("      {want:.3}  {who} toward {about}");
    }

    candidates.sort_by(|a, b| (b.0 * b.1).total_cmp(&(a.0 * a.1)));
    if candidates.is_empty() {
        println!(
            "\n  Nobody in this world both loathes somebody and has nothing left. Whether that\n  \
             is a society or a broken gate is decided by the two figures above: if either\n  \
             half never happens at all, the conjunction was never the reason."
        );
    } else {
        println!("\n  who could:");
        for (hate, gone, who, about) in candidates.iter().take(8) {
            println!("    {who:<22} hates {about:<22} {hate:.2}, spent {gone:.2}");
        }
    }
}
