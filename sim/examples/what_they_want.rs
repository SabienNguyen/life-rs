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
//! - **Whether the seven are all reachable.** A longing nobody ever has is a longing that is
//!   not there, whatever the source says. And — added after §36.6 — whether each is on the
//!   *same scale* as the others, which is a separate question and the one that matters: a
//!   longing can be common, win sometimes, and still never clear the floor that lets it change
//!   a decision.
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
    // How strongly each longing is felt across everybody, whether or not it is the one that
    // wins — the check that they are on a common scale. See below.
    let (mut reach, mut reached) = ([0.0f64; Dream::COUNT], [0.0f32; Dream::COUNT]);
    let mut clears = [0usize; Dream::COUNT];

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
            // Every longing's strength for this person, not only the winning one. Counting
            // wins alone hid §36.6's failure twice over: a longing can win for the people who
            // have no other longing at all, and so read as present in the table above while
            // doing nothing anywhere a decision is made. A number is on the same scale as its
            // siblings or it is not, and only the strengths side by side can say which.
            let all = dreams::longings(person, &at, now);
            for (which, strength) in all.iter().enumerate() {
                reach[which] += *strength as f64;
                reached[which] = reached[which].max(*strength);
                if *strength > dreams::WORTH_WANTING {
                    clears[which] += 1;
                }
            }
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

    // The most useful block here, and the one added last. Seven longings are only comparable
    // if they are on one scale, and nothing about the code makes them so — each is written by
    // hand and the arithmetic is different in every one.
    //
    // Read all three columns, and print all three for a reason §36.6 paid for. The mean says
    // whether a longing is felt; the maximum and the floor-clearing share say whether it can
    // ever *do* anything, which is a different question. An earlier version of this printed the
    // mean alone, and the mean alone reported §36.6's envy as the second-healthiest longing of
    // the seven while it had two errors inside it cancelling each other out — an inflated gap
    // times a shape that could not reach its siblings' range. **A composite cannot validate its
    // parts**, and one figure of merit per longing is a composite.
    println!("\n  how strongly each is felt by everybody, won or not — the common-scale check:");
    for dream in Dream::ALL {
        println!(
            "  {:<18} mean {:>6.3}   strongest anybody {:>6.3}   clears the floor for {:>5.1}% of adults",
            dream.label(),
            reach[dream as usize] / adults.max(1) as f64,
            reached[dream as usize],
            100.0 * clears[dream as usize] as f32 / adults.max(1) as f32
        );
    }

    // And the one number that justified dropping winner-take-all (§36.6): how many longings
    // clear the floor against how many were ever heard. Printed rather than worked out by hand
    // from the percentages above, because a figure quoted in a document is a figure that goes
    // stale the next time anybody touches the arithmetic.
    let clearing: usize = clears.iter().sum();
    println!(
        "\n  {clearing} longings clear the floor across {adults} adults, and {wanting} were heard — \
         {} discarded\n  for the sole reason that the same person wanted something else more \
         (§36.6, on why all seven get a say)",
        clearing.saturating_sub(wanting)
    );

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
