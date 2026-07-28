//! Is anybody in this world a Caesar?
//!
//! A world can have great men in two different senses. One is *distinction* — somebody far
//! out at the tail of what anybody has. The other is **reach**: somebody whose life changes
//! what happens to people who never met them. The second is what "a Caesar" means, and the
//! two come apart. This measures both.

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

fn main() {
    let mut world = World::genesis(WorldSeed::from_u128(0x221), 120);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);
    world.run_for(Duration::from_years(std::env::var("YEARS").ok().and_then(|v| v.parse().ok()).unwrap_or(120)));
    let now = world.now();

    // Distinction: how far the best-off adult is from the middle of the pack.
    let mut standing: Vec<(f32, String, usize)> = world
        .people
        .iter()
        .filter(|(_, p)| p.is_alive() && p.has_matured())
        .map(|(id, p)| {
            (
                p.standing(),
                p.name.clone(),
                world.bonds.of(id).filter(|(_, t)| t.allied()).count(),
            )
        })
        .collect();
    standing.sort_by(|a, b| b.0.total_cmp(&a.0));
    let median = standing[standing.len() / 2].0;
    println!("{} adults alive", standing.len());
    println!("  median standing        {median:.3}");
    println!("  the five best off:");
    for (has, name, allies) in standing.iter().take(5) {
        println!(
            "    {name:<22} {has:.3}  = {:.1}x the median, {allies} stand with them",
            has / median.max(1e-6)
        );
    }

    // Reach: how much of the world one person's ties touch, and whether any of it crosses
    // out of the place they live in. A patron among a hundred and fifty neighbours is a
    // considerable man in a village; it is not the same as mattering to a country.
    let alive = standing.len().max(1);
    let (top_allies, top_name) = standing
        .iter()
        .max_by_key(|(_, _, a)| *a)
        .map(|(_, n, a)| (*a, n.clone()))
        .unwrap_or((0, String::new()));
    println!(
        "\n  most connected         {top_name} — {top_allies} allies, {:.0}% of everybody alive",
        100.0 * top_allies as f32 / alive as f32
    );

    // And the only thing anybody can do that outlives them: work something out. Who ever
    // has, and how many people that reached.
    let advances: Vec<String> = world
        .chronicle
        .iter()
        .filter_map(|r| match r.kind {
            sim::Happening::PersonWorksItOut { person, trade } => Some(format!(
                "{} ({trade:?})",
                world
                    .people
                    .get(person)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "somebody".into())
            )),
            _ => None,
        })
        .collect();
    println!(
        "\n  ever changed what was possible for anybody else: {}",
        if advances.is_empty() {
            "nobody".to_string()
        } else {
            format!("{} people — {}", advances.len(), advances.join(", "))
        }
    );
}
