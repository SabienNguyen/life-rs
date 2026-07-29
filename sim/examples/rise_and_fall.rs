//! A country, from nothing to nothing.
//!
//! A country here is not drawn on a map and nobody declares one. It is the set of places whose
//! people can reach each other *and* share enough of their ways to count as the same — so it
//! can grow by settling a neighbour, split when the ways drift apart, and end by having nobody
//! left in it. This walks a world a decade at a time and prints what each country was, so the
//! rising and the falling can be read rather than asserted.
//!
//!     SEED=5ee YEARS=260 cargo run --release --example rise_and_fall

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

fn main() {
    let seed = std::env::var("SEED")
        .ok()
        .and_then(|v| u128::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x5ee);
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(260);
    let founders: usize = std::env::var("FOUNDERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);

    let mut world = World::genesis(WorldSeed::from_u128(seed), founders);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);

    println!("seed {seed:x}, {founders} founders\n");
    for decade in 0..(years / 10) {
        world.run_for(Duration::from_years(10));
        let year = (decade + 1) * 10;
        let countries = world.countries();
        let line: Vec<String> = countries
            .iter()
            .map(|country| {
                let souls: u32 = country
                    .places
                    .iter()
                    .filter_map(|at| world.souls_at(*at))
                    .sum();
                // What the ground gives a head, averaged over the places that have anybody in
                // them — a country nobody lives in has no answer rather than a zero.
                let lived_in: Vec<f32> = country
                    .places
                    .iter()
                    .filter(|at| world.souls_at(**at).unwrap_or(0) > 0)
                    .filter_map(|at| world.place_at(*at))
                    .filter_map(|id| world.places.get(id))
                    .map(|p| p.fortune)
                    .collect();
                let fortune = if lived_in.is_empty() {
                    f32::NAN
                } else {
                    lived_in.iter().sum::<f32>() / lived_in.len() as f32
                };
                let quarters: Vec<&str> = country
                    .places
                    .iter()
                    .filter(|at| world.souls_at(**at).unwrap_or(0) > 0)
                    .filter_map(|at| world.place_named(*at))
                    .collect();
                format!(
                    "{:<12} {souls:>5} souls  fortune {fortune:>5.2}  {}",
                    country.name,
                    quarters.join(", ")
                )
            })
            .collect();
        println!("year {year:>4}  ({} living)", world.living());
        for entry in line {
            println!("    {entry}");
        }
    }

    // And the record for whichever quarter swung hardest, from the same run — because a
    // decade table and a chronicle taken from two runs of the same seed are two different
    // worlds if anything at all differed between them, the detail budget included.
    let watched = std::env::var("WATCH").unwrap_or_default();
    if watched.is_empty() {
        return;
    }
    let Some(place) = world
        .places
        .iter()
        .find(|(_, p)| p.name == watched)
        .map(|(id, _)| id)
    else {
        println!("\nno quarter called {watched}");
        return;
    };
    let calendar = world.planets.iter().next().map(|(_, p)| p.calendar);
    println!("\n-- {watched}, from the record --");
    for record in world.chronicle.iter() {
        let names = match record.kind {
            sim::Happening::PersonMoves { person, to } if to == place => Some(format!(
                "{} arrives",
                world
                    .people
                    .get(person)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "somebody".to_string())
            )),
            sim::Happening::PlaceChanges { place: at, into } if at == place => {
                Some(format!("reads as {into:?}"))
            }
            _ => None,
        };
        if let Some(what) = names {
            let year = calendar.map(|c| c.date_at(record.at).year).unwrap_or(0);
            println!("  {year:>4}  {what}");
        }
    }
    if let Some(p) = world.places.get(place) {
        println!(
            "\n  {} at the end: {} living, room for {}, reads {:?}",
            p.name,
            world
                .society
                .households_in(place)
                .flat_map(|(_, h)| h.members.iter())
                .filter(|m| world.people.get(**m).is_some_and(|q| q.is_alive()))
                .count(),
            p.capacity,
            p.archetype()
        );
        println!(
            "  fortune {:.3}, gives a head {:.3}, short of food {:.3}",
            p.fortune, p.prosperity, p.want
        );
    }

    // Who is anybody there, and what do their people do differently. There is no doctrine in
    // this model and no religion in it at all — a culture here is the seven numbers §14
    // already kept as `norms`, which are literally how much of each day's doing a people
    // spends on each thing. So "their ways" is a report of behaviour, not of belief.
    let living: Vec<_> = world
        .society
        .households_in(place)
        .flat_map(|(_, h)| h.members.iter().copied())
        .filter(|m| world.people.get(*m).is_some_and(|q| q.is_alive()))
        .collect();

    let mut trades: std::collections::BTreeMap<&str, usize> = Default::default();
    for who in &living {
        if let Some(person) = world.people.get(*who) {
            *trades.entry(person.trade().label()).or_default() += 1;
        }
    }
    let mut mix: Vec<_> = trades.into_iter().collect();
    mix.sort_by(|a, b| b.1.cmp(&a.1));
    println!(
        "\n  what they do: {}",
        mix.iter()
            .map(|(t, n)| format!("{n} {t}"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut roles: std::collections::BTreeMap<String, usize> = Default::default();
    let mut named: Vec<(String, String, bonds::Role, f64, usize)> = Vec::new();
    for (who, _, role) in world.society_of(place) {
        *roles.entry(format!("{role:?}")).or_default() += 1;
        if matches!(role, bonds::Role::Elder | bonds::Role::Patron) {
            if let Some(person) = world.people.get(who) {
                let title = world
                    .standing_of(who)
                    .map(|(_, word)| word)
                    .unwrap_or_default();
                named.push((
                    person.name.clone(),
                    title,
                    role,
                    person.age(world.now()).years(),
                    world.bonds.of(who).filter(|(_, t)| t.allied()).count(),
                ));
            }
        }
    }
    let mut tally: Vec<_> = roles.into_iter().collect();
    tally.sort_by(|a, b| b.1.cmp(&a.1));
    println!(
        "  what they are: {}",
        tally
            .iter()
            .map(|(r, n)| format!("{n} {}", r.to_lowercase()))
            .collect::<Vec<_>>()
            .join(", ")
    );
    named.sort_by_key(|entry| std::cmp::Reverse(entry.4));
    println!("  who is looked to:");
    for (name, title, role, age, allies) in named.iter().take(6) {
        println!("    {name:<22} {title:<14} {role:?}, {age:.0} yr, {allies} stand with them");
    }

    if let Some(people) = world.people_of(place) {
        println!(
            "\n  their people: the {}, {} souls{}",
            people.name,
            people.souls,
            people
                .parent
                .map(|_| format!(", arose in year {}", people.arose))
                .unwrap_or_else(|| " (here from the founding)".to_string())
        );
        let names = ["eating", "drinking", "sleeping", "washing", "socialising", "working", "wandering"];
        let ways: Vec<String> = people
            .ways
            .iter()
            .enumerate()
            .map(|(at, w)| format!("{} {w:.2}", names[at]))
            .collect();
        println!("  their ways: {}", ways.join(", "));
    }
}
