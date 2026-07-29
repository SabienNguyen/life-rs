//! Do people settle into trades, or slosh between them?
//!
//! The chronicle only records a *settled* person changing trade, so it cannot see the young,
//! who do most of the moving. This follows everybody year by year instead and asks three
//! things: how often somebody goes back to the trade before last, how much of a year's
//! movement goes to one and the same trade — a herd, and the signature of a cobweb — and
//! whether a place still ends up with a sensible mix of hands.

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

fn main() {
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);
    let (mut moves, mut back, mut herd, mut flow) = (0usize, 0usize, 0usize, 0usize);
    for seed in [0x11u128, 0x21, 0x31, 0x221] {
        let mut world = World::genesis(WorldSeed::from_u128(seed), 120);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);

        let mut path: std::collections::BTreeMap<u64, Vec<usize>> = Default::default();
        let (mut seed_herd, mut seed_flow) = (0usize, 0usize);
        for _ in 0..years {
            let before: std::collections::BTreeMap<u64, usize> = world
                .people
                .iter()
                .filter(|(_, p)| p.is_alive())
                .map(|(id, p)| (id.to_bits(), p.trade() as usize))
                .collect();
            world.run_for(Duration::from_years(1));
            // Where this year's changes of trade went. If they nearly all went to the same
            // one, everybody read the same signal and acted on it together.
            let mut into = [0usize; 5];
            for (id, person) in world.people.iter() {
                if !person.is_alive() {
                    continue;
                }
                let now = person.trade() as usize;
                if before.get(&id.to_bits()).is_some_and(|was| *was != now) {
                    into[now] += 1;
                    path.entry(id.to_bits()).or_default().push(now);
                }
            }
            seed_flow += into.iter().sum::<usize>();
            seed_herd += into.iter().max().copied().unwrap_or(0);
        }
        let (mut m, mut b) = (0usize, 0usize);
        for steps in path.values() {
            m += steps.len();
            b += (2..steps.len()).filter(|i| steps[*i] == steps[i - 2]).count();
        }
        // What the world settled into: how many hands each trade holds, worst place first.
        let mix: Vec<String> = ["farm", "hew", "smith", "cook", "keep"]
            .iter()
            .enumerate()
            .map(|(t, name)| {
                let n = world
                    .people
                    .iter()
                    .filter(|(_, p)| p.is_alive() && p.trade() as usize == t)
                    .count();
                format!("{name} {n}")
            })
            .collect();
        println!(
            "{seed:>5x}: {m:>5} changes, {b:>4} straight back, herd {:>3.0}%   {}",
            100.0 * seed_herd as f32 / seed_flow.max(1) as f32,
            mix.join("  ")
        );
        moves += m;
        back += b;
        herd += seed_herd;
        flow += seed_flow;
    }
    println!(
        "\n{moves} changes of trade, {:.0}% straight back, {:.0}% of a year's changes to one trade",
        100.0 * back as f32 / moves.max(1) as f32,
        100.0 * herd as f32 / flow.max(1) as f32
    );
}
