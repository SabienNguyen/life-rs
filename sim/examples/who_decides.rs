//! There is no government. This reads out what there is instead.
//!
//! §24.4 leaves out the state deliberately — no law, no taxation, no army, no border
//! anybody could be stopped at. But something decides who gets the good land and whose
//! household is admitted, and §25 says what: standing, plus what your allies inside a place
//! will lend you. This walks a world and prints that, so the claim can be checked against a
//! town rather than believed.
//!
//!     cargo run --release --example who_decides [seed] [years]

use sim::{Detail, World};
use sim_core::{Duration, Salience, WorldSeed};

fn main() {
    let seed: u128 = arg(1).unwrap_or(0x5ee);
    let years: u64 = arg(2).unwrap_or(150);

    let mut world = World::genesis(WorldSeed::from_u128(seed as u128), 120);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);
    world.run_for(Duration::from_years(years));
    let now = world.now();

    let countries = world.countries();
    println!("{} alive in {} countries, year {years}", world.living(), countries.len());
    for country in &countries {
        let souls: u32 = country.places.iter().filter_map(|p| world.souls_at(*p)).sum();
        let quarters: Vec<&str> = country.places.iter().filter_map(|p| world.place_named(*p)).collect();
        println!("  {:<14} {souls:>4} souls   {}", country.name, quarters.join(", "));
    }

    let Some(country) = countries
        .iter()
        .max_by_key(|c| c.places.iter().filter_map(|p| world.souls_at(*p)).sum::<u32>())
    else {
        return;
    };
    println!("\n=== {} ===", country.name);

    for at in &country.places {
        let (Some(name), Some(id)) = (world.place_named(*at).map(str::to_owned), world.place_at(*at))
        else {
            continue;
        };
        let here: Vec<person::PersonId> = world
            .society
            .households_in(id)
            .flat_map(|(_, h)| h.members.iter().copied())
            .filter(|m| {
                world.people.get(*m).is_some_and(|p| p.is_alive() && !p.stage(now).is_dependent())
            })
            .collect();
        if here.len() < 3 {
            continue;
        }
        println!(
            "\n-- {name}: {} adults, {}",
            here.len(),
            if world.detail_of(id) == Detail::Full { "watched" } else { "unwatched" }
        );

        // What each of them can bring to bear, which is the only thing in this world that
        // settles anything: what they have, plus what their allies here would lend.
        let mut weight: Vec<(f32, f32, person::PersonId)> = here
            .iter()
            .filter_map(|w| {
                let own = world.people.get(*w)?.standing();
                Some((own + world.backing(&[*w], id), own, *w))
            })
            .collect();
        weight.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.2.cmp(&b.2)));

        let read = world.society_of(id);
        let mut tally: std::collections::BTreeMap<&str, usize> = Default::default();
        for (_, _, role) in &read {
            *tally.entry(role.label()).or_default() += 1;
        }
        println!(
            "   {}",
            tally
                .iter()
                .map(|(role, n)| format!("{n} {role}"))
                .collect::<Vec<_>>()
                .join(", ")
        );

        for (total, own, who) in weight.iter().take(6) {
            let Some(p) = world.people.get(*who) else { continue };
            let title = world
                .standing_of(*who)
                .map(|(role, word)| format!("{word} ({})", role.label()))
                .unwrap_or_default();
            let allies = world.bonds.of(*who).filter(|(_, t)| t.allied()).count();
            let owed: f32 = world.bonds.of(*who).map(|(_, t)| t.debt.max(0.0)).sum();
            let owes: f32 = world.bonds.of(*who).map(|(_, t)| (-t.debt).max(0.0)).sum();
            let parent = world
                .society
                .parents_of(*who)
                .and_then(|(_, f)| world.people.get(f))
                .map(|f| f.name.clone())
                .unwrap_or_else(|| "—".into());
            println!(
                "   {:<20} {:<26} {:>3.0}  has {own:.2}  with friends {total:.2}  \
{allies:>2} allies  owed {owed:>4.0}d  owes {owes:>3.0}d  child of {parent}{}",
                p.name,
                title,
                p.age(now).years(),
                if p.is_mentored() { format!("  patron {:.1}x", p.patronage()) } else { String::new() },
            );
        }

        // The ledger of the one at the top. Every day in it was a bad year somebody was
        // carried through — there is no other way to be owed anything here.
        if let Some((_, _, first)) = weight.first()
            && let Some(p) = world.people.get(*first)
        {
            println!("   what {} is owed, and by whom:", p.name);
            let mut ledger: Vec<(f32, String, f32)> = world
                .bonds
                .of(*first)
                .filter(|(_, t)| t.debt > 1.0)
                .filter_map(|(other, t)| Some((t.debt, world.people.get(other)?.name.clone(), t.warmth)))
                .collect();
            ledger.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (debt, name, warmth) in ledger.iter().take(6) {
                println!("     {name:<20} {debt:>5.0} days   and they feel {warmth:+.2} about it");
            }
            if let Some(by) = world.life_of(*first).find_map(|r| match r.kind {
                sim::Happening::PersonMentored { person, by } if person == *first => Some(by),
                _ => None,
            }) {
                let tie = world.bonds.tie(*first, by);
                println!(
                    "     taken up by {} — warmth {:+.2}, {:.0} days still owed",
                    world.people.get(by).map(|q| q.name.clone()).unwrap_or_default(),
                    tie.warmth,
                    (-tie.debt).max(0.0),
                );
            }
        }

        let circles = bonds::circles::circles(&world.bonds, &here);
        println!("   {} circles, largest {}", circles.len(), circles.first().map_or(0, |c| c.members.len()));
        for circle in circles.iter().take(3) {
            let names: Vec<&str> = circle
                .members
                .iter()
                .filter_map(|m| world.people.get(*m).map(|p| p.name.as_str()))
                .collect();
            println!("     [{:.2}] {}", circle.cohesion, names.join(", "));
        }
    }
}

fn arg<T: std::str::FromStr>(at: usize) -> Option<T> {
    std::env::args().nth(at)?.parse().ok()
}
