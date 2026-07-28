//! What everybody in a world is after, before any of it changes anything.
//!
//! §36's dreams are readings — computed from what somebody carries and where they have ended
//! up, never stored — and the first question about a reading is not whether it works but
//! whether it *distinguishes anybody*. A longing that everybody has in equal measure is a
//! constant with a name, and the project has shipped two of those already (§30.5's dead
//! crowding term, §17.2.3's belief on a tie), both of which read as mechanisms for months.
//!
//! So this runs before the dreams are wired to a single decision. It asks three things:
//!
//! - **How many people want anything at all.** Most should not. A world in which everybody is
//!   driven is one where being driven means nothing.
//! - **Whether the six are all reachable.** A longing nobody ever has is a longing that is not
//!   there, whatever the source says.
//! - **Whether the same person wants different things at different ages** — because the whole
//!   argument for a reading over a field is that it changes when a life does, and if the
//!   distribution is the same at thirty and at sixty then nothing has been gained over
//!   drawing one at birth.
//!
//!     cargo run --release --example what_they_want

use person::dreams::{self, Dream};
use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

const SEEDS: [u128; 3] = [0x11, 0x21, 0x221];

fn main() {
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);

    // By dream, and then by age band, so that "it changes with a life" is a thing this can
    // see rather than a thing the doc comment claims.
    let mut held = [0usize; Dream::COUNT];
    let mut by_age = [[0usize; Dream::COUNT]; 3];
    let (mut adults, mut wanting) = (0usize, 0usize);
    let mut strongest: Vec<(f32, String, Dream, f64)> = Vec::new();

    for seed in SEEDS {
        let mut world = World::genesis(WorldSeed::from_u128(seed), 120);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.run_for(Duration::from_years(years));
        let now = world.now();

        for (id, person) in world.people.iter() {
            if !person.is_alive() || !person.has_matured() {
                continue;
            }
            let Some(at) = world.what_they_have_come_to(id) else {
                continue;
            };
            adults += 1;
            let age = person.age(now).years();
            let band = if age < 35.0 {
                0
            } else if age < 55.0 {
                1
            } else {
                2
            };
            if let Some((dream, how_much)) = dreams::of(person, &at, now) {
                wanting += 1;
                held[dream as usize] += 1;
                by_age[band][dream as usize] += 1;
                strongest.push((how_much, person.name.clone(), dream, age));
            }
        }
    }

    println!("{} seeds, {years} years\n", SEEDS.len());
    println!(
        "  {wanting} of {adults} adults want something in particular  ({:.0}%)\n",
        100.0 * wanting as f32 / adults.max(1) as f32
    );
    for dream in Dream::ALL {
        println!(
            "  {:<18} {:>5}   {:>4.1}% of those who want anything",
            dream.label(),
            held[dream as usize],
            100.0 * held[dream as usize] as f32 / wanting.max(1) as f32
        );
    }

    println!("\n  and by age — the claim that a reading changes when a life does:");
    println!(
        "  {:<18} {:>8} {:>8} {:>8}",
        "", "under 35", "35 to 55", "over 55"
    );
    for dream in Dream::ALL {
        let share = |band: usize| {
            let total: usize = by_age[band].iter().sum();
            100.0 * by_age[band][dream as usize] as f32 / total.max(1) as f32
        };
        println!(
            "  {:<18} {:>7.1}% {:>7.1}% {:>7.1}%",
            dream.label(),
            share(0),
            share(1),
            share(2)
        );
    }

    strongest.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("\n  who wants it most:");
    for (how_much, name, dream, age) in strongest.iter().take(8) {
        println!("    {name:<24} {:<18} {how_much:.2}  at {age:.0}", dream.label());
    }
}
