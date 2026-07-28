//! What people are to each other, counted.
//!
//! A census of the relationships this world actually contains, rather than a list of the ones
//! a reader might expect. Everything here is read off the tie graph and the household record —
//! nothing is a label anybody assigned.

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

fn main() {
    let mut world = World::genesis(WorldSeed::from_u128(0x221), 160);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);
    world.run_for(Duration::from_years(
        std::env::var("YEARS").ok().and_then(|v| v.parse().ok()).unwrap_or(140),
    ));
    let now = world.now();

    let adults: Vec<_> = world
        .people
        .iter()
        .filter(|(_, p)| p.is_alive() && p.has_matured())
        .map(|(id, _)| id)
        .collect();
    let n = adults.len().max(1);

    // Ties, by what they are. Warmth runs from loathing at -1 to devotion at +1, so the tie
    // graph carries dislike as naturally as liking — nothing separate had to be built for it.
    let (mut allies, mut warm, mut cool, mut enemies, mut known) = (0, 0, 0, 0, 0);
    let (mut owed, mut most_allies, mut most_enemies) = (0, 0usize, 0usize);
    for who in &adults {
        let (mut a, mut e) = (0usize, 0usize);
        for (_, tie) in world.bonds.of(*who) {
            if !tie.holds() {
                continue;
            }
            known += 1;
            if tie.allied() {
                allies += 1;
                a += 1;
            }
            if tie.warmth > 0.25 {
                warm += 1;
            } else if tie.warmth < -0.25 {
                enemies += 1;
                e += 1;
            } else if tie.warmth < 0.0 {
                cool += 1;
            }
            if tie.debt.abs() > 0.5 {
                owed += 1;
            }
        }
        most_allies = most_allies.max(a);
        most_enemies = most_enemies.max(e);
    }

    println!("{n} adults\n");
    println!("  people they know      {:>6.1} each", known as f32 / n as f32);
    println!("    of whom allies      {:>6.1}   known, liked, and not in your debt", allies as f32 / n as f32);
    println!("    warmly regarded     {:>6.1}", warm as f32 / n as f32);
    println!("    coolly              {:>6.1}", cool as f32 / n as f32);
    println!("    actively disliked   {:>6.1}   warmth below -0.25", enemies as f32 / n as f32);
    println!("    owe or are owed     {:>6.1}   days of help, from a bad year", owed as f32 / n as f32);
    println!("  the most befriended has {most_allies}, the most resented {most_enemies}");

    // Households: who lives with whom, and who has paired with whom.
    let partnered = adults
        .iter()
        .filter(|id| world.society.partner_of(**id).is_some())
        .count();
    let widowed = adults
        .iter()
        .filter(|id| {
            world.society.partner_of(**id).is_none()
                && world
                    .chronicle
                    .iter()
                    .any(|r| matches!(r.kind, sim::Happening::PersonPairs { person, .. } if person == **id))
        })
        .count();
    let mut pairings = 0;
    let mut repartnered: std::collections::BTreeMap<u64, usize> = Default::default();
    for record in world.chronicle.iter() {
        if let sim::Happening::PersonPairs { person, .. } = record.kind {
            pairings += 1;
            *repartnered.entry(person.to_bits()).or_default() += 1;
        }
    }
    let more_than_once = repartnered.values().filter(|c| **c > 1).count();
    println!("\n  living with a partner {partnered:>6}   of {n}");
    println!("  paired and now not    {widowed:>6}   partner died; nobody here separates");
    println!("  pairings ever         {pairings:>6}, of which {more_than_once} people paired more than once");

    // Kin.
    let with_kids = adults
        .iter()
        .filter(|id| !world.society.children_of(**id).is_empty())
        .count();
    println!("  have children         {with_kids:>6}");
    let mentored = world
        .people
        .iter()
        .filter(|(_, p)| p.is_alive() && p.is_mentored())
        .count();
    println!("  taken up by a patron  {mentored:>6}   the largest single fact about a life here");
    let _ = now;
}
