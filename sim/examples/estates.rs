//! Does anybody actually own anything?
use sim::World;
use sim_core::{Duration, Salience, WorldSeed};
fn main() {
    let mut world = World::genesis(WorldSeed::from_u128(0x11), 160);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);
    world.run_for(Duration::from_years(std::env::var("YEARS").ok().and_then(|v| v.parse().ok()).unwrap_or(120)));
    let mut estates: Vec<f32> = world.people.iter()
        .filter(|(_, p)| p.is_alive() && p.has_matured())
        .map(|(_, p)| p.estate()).collect();
    estates.sort_by(|a, b| b.total_cmp(a));
    let n = estates.len().max(1);
    let with = estates.iter().filter(|e| **e > 1e-6).count();
    println!("{n} adults, {with} own anything at all ({:.0}%)", 100.0 * with as f32 / n as f32);
    println!("  largest estate  {:.3}", estates.first().copied().unwrap_or(0.0));
    println!("  median          {:.3}", estates[n / 2]);
    println!("  mean            {:.3}", estates.iter().sum::<f32>() / n as f32);
    let standing: f32 = world.people.iter().filter(|(_, p)| p.is_alive() && p.has_matured())
        .map(|(_, p)| p.standing()).sum::<f32>() / n as f32;
    println!("  mean standing   {standing:.3}   <- what an estate is competing with");
}
