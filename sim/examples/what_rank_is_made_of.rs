//! What social rank in this world is actually a ranking of.
//!
//! `repute` is a percentile: everybody's mean `regard` is sorted and their position in that
//! order, normalised, becomes their rank. That is a good design — it cannot saturate and it
//! cannot drift with the units of the thing underneath it — and it has one property worth
//! checking, which is that **a percentile of a constant is not a percentile of anything.**
//!
//! `regard` was measured at a mean absolute value of 0.0015, moved off zero on 2.3% of live
//! ties. If that leaves a large share of people with a mean regard of *exactly* zero, then
//! sorting them puts them in the order decided by the tie-break:
//!
//!     said.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
//!
//! and `a.0` is a `PersonId` — an arena handle, handed out in the order people were born. The
//! social hierarchy of a world would then be, for most of its people, a ranking by birth order
//! wearing the name of reputation. Every mechanism reading `rank` — `Dream::ToRise`,
//! `ToBeLookedTo`, household sorting, who a patron opens a door for — would be reading that.
//!
//! This is the §30.5 and §17.2.3 failure once more: a quantity that has a name, is read all
//! over, and is a constant. Both of those read as mechanisms for months.
//!
//!     cargo run --release --example what_rank_is_made_of

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

const SEEDS: [u128; 3] = [0x11, 0x21, 0x221];

fn main() {
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);

    for seed in SEEDS {
        let mut world = World::genesis(WorldSeed::from_u128(seed), 120);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.run_for(Duration::from_years(years));

        // Everybody's mean regard, exactly as `reckon_bonds` computes it before sorting.
        let mut mean_regard: Vec<(person::PersonId, f32)> = world
            .bonds
            .everybodys_repute()
            .into_iter()
            .map(|(who, (total, holders))| (who, total / holders.max(1) as f32))
            .collect();
        mean_regard.retain(|(who, _)| {
            world
                .people
                .get(*who)
                .is_some_and(|p| p.is_alive() && p.has_matured())
        });

        let flat = mean_regard.iter().filter(|(_, r)| *r == 0.0).count();
        let all = mean_regard.len();

        // How many share their value with somebody else — the people whose order the
        // tie-break decides, which is the larger question. Exact zero is the common case but
        // any repeated value has the same problem.
        let mut seen: std::collections::BTreeMap<u32, usize> = Default::default();
        for (_, regard) in &mean_regard {
            *seen.entry(regard.to_bits()).or_default() += 1;
        }
        let tied: usize = seen.values().filter(|n| **n > 1).sum();

        // And whether the resulting rank is just birth order — **read off the world** rather
        // than recomputed here. An instrument that reimplements the thing it is checking
        // cannot see a change to it, which is how the first version of this reported the fix
        // as having done nothing.
        let mut order: Vec<person::PersonId> = mean_regard.iter().map(|(w, _)| *w).collect();
        order.sort();
        let n = order.len() as f64;
        let (mut sx, mut sy, mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (birth, who) in order.iter().enumerate() {
            let (x, y) = (world.repute_of(*who) as f64, birth as f64);
            sx += x;
            sy += y;
            sxy += x * y;
            sxx += x * x;
            syy += y * y;
        }
        let spearman = (n * sxy - sx * sy)
            / (((n * sxx - sx * sx) * (n * syy - sy * sy)).sqrt()).max(f64::MIN_POSITIVE);
        // How many distinct ranks the world actually hands out. If a third of everybody now
        // shares one value, that is the honest reading — the community has no opinion about
        // them — and it should be visible rather than hidden behind a smooth-looking spread.
        let steps: std::collections::BTreeSet<u32> =
            order.iter().map(|w| world.repute_of(*w).to_bits()).collect();

        // What the rank distribution actually looks like among the living. A percentile is
        // uniform by construction *over whoever was sorted*, so if the sorted set is not the
        // set being read — the dead are in it, or people with no ties are missing — the living
        // can sit anywhere in it. §36's `ToRise` reads `1 - rank` and fell by three quarters
        // when this changed, which is either that fact or a real effect, and only the shape
        // can say which.
        let mut ranks: Vec<f32> = order.iter().map(|w| world.repute_of(*w)).collect();
        ranks.sort_by(f32::total_cmp);
        let rank_at = |q: f32| ranks[((ranks.len() as f32 - 1.0) * q) as usize];
        let mean_rank = ranks.iter().sum::<f32>() / ranks.len().max(1) as f32;
        let sorted_over = world.bonds.everybodys_repute().len();

        // What a living looks like at each age. §42.4 rates people by their means against one
        // fixed middle, which makes `regard` a synonym for wealth and therefore a tax on being
        // young — and patronage, which requires an elder to think well of you, fell 40% because
        // the people who seek patrons are by construction the people with least. What anybody
        // is actually rated against is what is normal *for somebody their age*, and this is
        // that number rather than a guess at it.
        {
            let now = world.now();
            let whole = life::Mortality::HUMAN.median_lifespan();
            let mut bands = [(0.0f64, 0usize); 5];
            for (who, person) in world.people.iter() {
                if !person.is_alive() || !person.has_matured() {
                    continue;
                }
                let _ = who;
                let through = (person.age(now).years() / whole).clamp(0.0, 1.0);
                let band = ((through * 5.0) as usize).min(4);
                bands[band].0 += person.means() as f64;
                bands[band].1 += 1;
            }
            print!("  means by fifth of a life   ");
            for (total, n) in bands {
                print!("{:>6.2}", total / n.max(1) as f64);
            }
            println!();
        }

        println!("seed {seed:x}, {all} adults");
        println!(
            "  rank of the living          mean {mean_rank:.3}, p10 {:.2}, p50 {:.2}, p90 {:.2}",
            rank_at(0.1),
            rank_at(0.5),
            rank_at(0.9)
        );
        println!(
            "  ties are held about        {sorted_over:>5}   people in all; {all} of them are grown"
        );
        println!(
            "  mean regard exactly zero    {flat:>5}   ({:.1}% — ranked by the tie-break alone)",
            100.0 * flat as f32 / all.max(1) as f32
        );
        println!(
            "  sharing a value with anyone {tied:>5}   ({:.1}%)",
            100.0 * tied as f32 / all.max(1) as f32
        );
        println!("  rank against birth order    {spearman:>5.3}   (1.000 means rank *is* birth order)");
        println!(
            "  distinct ranks handed out   {:>5}   of {all} adults\n",
            steps.len()
        );
    }
}
