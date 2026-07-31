//! §42's findings with names on them.
//!
//! Everything in §42 is a distribution: regard moved off zero on 96.6% of ties, disagreement
//! about a person went from 0.001 to 0.048, concentration rose from 0.54 to 0.61. Those are
//! the right numbers to *judge* a mechanism by, and they are useless for knowing whether it
//! produced anything worth having. A society is not a histogram, and "the world has become
//! stratified" is a sentence that could describe a real change or a rounding artefact.
//!
//! So this walks one world and finds the people the numbers are about — the man everybody has
//! turned against, the man half the town rates and half does not, the quarter that has become
//! where the well-regarded live, and the young person somebody opened a door for. It asserts
//! nothing. It is the counterpart to `vitals`: that one asks whether a change is real, this
//! one asks whether it is any good.
//!
//! §30.4's alternating-towns fault was found this way — by reading one life end to end in the
//! atlas and seeing the same two place names for twenty years — after the aggregates had
//! reported the world healthy for months.
//!
//!     cargo run --release --example what_it_looks_like

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

fn main() {
    let seed = std::env::var("SEED")
        .ok()
        .and_then(|v| u128::from_str_radix(v.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x21);
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);

    let mut world = World::genesis(WorldSeed::from_u128(seed), 120);
    world.record_only(Salience::Pivotal);
    world.set_detail_budget(100_000);
    world.run_for(Duration::from_years(years));
    let now = world.now();

    let name = |id| {
        world
            .people
            .get(id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "somebody".into())
    };
    let where_they_live = |id| {
        world
            .society
            .place_of(id)
            .and_then(|p| world.places.get(p))
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "nowhere".into())
    };

    let grown: Vec<_> = world
        .people
        .iter()
        .filter(|(_, p)| p.is_alive() && p.has_matured())
        .map(|(id, p)| (id, p.name.clone()))
        .collect();

    println!(
        "seed {seed:x}, {years} years, {} living, {} of them grown\n",
        world.living(),
        grown.len()
    );

    // ── The man the town has turned against ───────────────────────────────────────────────
    //
    // What ostracism looks like from outside. Being disliked is ordinary — everybody is
    // disliked by somebody — so what is being looked for is *concentration*: somebody a large
    // share of the people who know them at all have turned against.
    println!("── Somebody the town has turned against ──\n");
    let mut outcasts: Vec<(f32, usize, usize, person::PersonId)> = Vec::new();
    for (id, _) in &grown {
        let (mut against, mut held) = (0usize, 0usize);
        for (_, tie) in world.bonds.of(*id) {
            if !tie.holds() {
                continue;
            }
            held += 1;
            if tie.warmth < -0.05 {
                against += 1;
            }
        }
        if held >= 12 {
            outcasts.push((against as f32 / held as f32, against, held, *id));
        }
    }
    outcasts.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (share, against, held, id) in outcasts.iter().take(3) {
        let person = world.people.get(*id).unwrap();
        println!(
            "  {:<24} {:>3}, of {} at {}",
            name(*id),
            person.age(now).years() as u32,
            where_they_live(*id),
            format_args!("rank {:.2}", world.repute_of(*id))
        );
        println!(
            "      known to {held}, and {against} of them have turned against \
             them ({:.0}%)",
            100.0 * share
        );
        // Who, and how sour. The names matter: an outcast with twenty faint dislikes is a
        // different thing from one with three people who loathe them.
        let mut sourest: Vec<(f32, person::PersonId)> = world
            .bonds
            .of(*id)
            .filter(|(_, t)| t.holds() && t.warmth < -0.05)
            .map(|(other, t)| (t.warmth, other))
            .collect();
        sourest.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (warmth, other) in sourest.iter().take(3) {
            println!("        {warmth:>6.2} toward {}", name(*other));
        }
        println!();
    }

    // ── The man half the town rates ───────────────────────────────────────────────────────
    //
    // §42's whole point. Before it, `divided` was 0.001 and this section could not have
    // existed: everybody's reputation was one number and the world agreed on it exactly.
    println!("── Somebody the town cannot agree about ──\n");
    let divided = world.bonds.how_divided();
    let mut contested: Vec<(f32, f32, u32, person::PersonId)> = divided
        .into_iter()
        .filter(|(who, (_, _, holders))| {
            *holders >= 10
                && world
                    .people
                    .get(*who)
                    .is_some_and(|p| p.is_alive() && p.has_matured())
        })
        .map(|(who, (mean, spread, holders))| (spread, mean, holders, who))
        .collect();
    contested.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (spread, mean, holders, id) in contested.iter().take(3) {
        let person = world.people.get(*id).unwrap();
        println!(
            "  {:<24} {:>3}, of {}",
            name(*id),
            person.age(now).years() as u32,
            where_they_live(*id)
        );
        println!(
            "      {holders} people hold an opinion; it averages {mean:+.2} and they \
             disagree by {spread:.2}"
        );
        let mut opinions: Vec<(f32, person::PersonId)> = Vec::new();
        for (holder, _) in &grown {
            let tie = world.bonds.tie(*holder, *id);
            if tie.holds() {
                opinions.push((tie.regard, *holder));
            }
        }
        opinions.sort_by(|a, b| b.0.total_cmp(&a.0));
        if let Some((best, who)) = opinions.first() {
            println!("        best of them  {best:+.2}  {}", name(*who));
        }
        if let Some((worst, who)) = opinions.last() {
            println!("        worst of them {worst:+.2}  {}", name(*who));
        }
        println!();
    }

    // Is that disagreement, or is it regard still on its way? Every one of the most-divided
    // people above is twenty-two. `regard` starts at zero and walks toward what somebody is
    // worth at a rate gated on `known`, so two holders who know a young person to different
    // degrees are at different points along the *same* path — which reads as spread and is
    // nothing of the kind. If §42's `divided` is really convergence lag then it decays with
    // age, and the world does not disagree about anybody after all.
    println!("\n  is that disagreement, or regard still arriving?");
    let mut by_age = [(0.0f32, 0usize); 4];
    for (id, _) in &grown {
        let Some((_, spread, holders)) = world.bonds.how_divided().get(id).copied() else {
            continue;
        };
        if holders < 10 {
            continue;
        }
        let age = world.people.get(*id).map(|p| p.age(now).years()).unwrap_or(0.0);
        let band = if age < 30.0 {
            0
        } else if age < 45.0 {
            1
        } else if age < 60.0 {
            2
        } else {
            3
        };
        by_age[band].0 += spread;
        by_age[band].1 += 1;
    }
    print!("    spread by age  ");
    for (label, (total, n)) in ["under 30", "30-45", "45-60", "over 60"]
        .iter()
        .zip(by_age.iter())
    {
        print!("{label} {:.3} ({n})   ", total / (*n).max(1) as f32);
    }
    println!();

    // ── Where the well-regarded live ──────────────────────────────────────────────────────
    //
    // The stratification §42.6 measured, as a place you could walk into. `backing` gates
    // admission on `repute`, and until §42 that gate was reading arena order, so this table
    // would have been noise.
    println!("── Where the well-regarded ended up ──\n");
    let mut quarters: Vec<(f32, f32, usize, String)> = Vec::new();
    for place in world.places.ids() {
        let here: Vec<_> = grown
            .iter()
            .filter(|(id, _)| world.society.place_of(*id) == Some(place))
            .map(|(id, _)| *id)
            .collect();
        if here.len() < 5 {
            continue;
        }
        let rank = here.iter().map(|id| world.repute_of(*id)).sum::<f32>() / here.len() as f32;
        let means = here
            .iter()
            .filter_map(|id| world.people.get(*id).map(|p| p.means()))
            .sum::<f32>()
            / here.len() as f32;
        let named = world
            .places
            .get(place)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        quarters.push((rank, means, here.len(), named));
    }
    quarters.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("  {:<22} {:>7} {:>8} {:>7}", "", "grown", "rank", "means");
    for (rank, means, held, named) in &quarters {
        println!("  {named:<22} {held:>7} {rank:>8.2} {means:>7.2}");
    }
    if let (Some(top), Some(bottom)) = (quarters.first(), quarters.last()) {
        println!(
            "\n  {} against {}: rank {:.2} to {:.2}, means {:.2} to {:.2}",
            top.3, bottom.3, top.0, bottom.0, top.1, bottom.1
        );
    }

    // ── A door that opened ────────────────────────────────────────────────────────────────
    println!("\n── A door somebody opened ──\n");
    let mut shown = 0;
    for (id, _) in &grown {
        let person = world.people.get(*id).unwrap();
        if !person.is_mentored() || shown >= 3 {
            continue;
        }
        // Who would have backed them: the best-placed person who thinks well of them, which
        // is what `seek_patron` scores. Read rather than recorded, so it is the same walk.
        let best = world
            .bonds
            .of(*id)
            .filter(|(_, t)| t.holds())
            .filter_map(|(other, _)| {
                let them = world.people.get(other)?;
                let theirs = world.bonds.tie(other, *id);
                let worth = them.standing() * theirs.known * (theirs.warmth + theirs.regard).max(0.0);
                (worth > 0.0 && them.is_alive()).then_some((worth, other))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0));
        let Some((worth, patron)) = best else { continue };
        println!(
            "  {:<24} {:>3}, of {}, rank {:.2}",
            name(*id),
            person.age(now).years() as u32,
            where_they_live(*id),
            world.repute_of(*id)
        );
        println!(
            "      vouched for by {} — rank {:.2}, and holds {:+.2} warmth and {:+.2} regard \
             toward them (worth {worth:.2})",
            name(patron),
            world.repute_of(patron),
            world.bonds.tie(patron, *id).warmth,
            world.bonds.tie(patron, *id).regard
        );
        shown += 1;
    }
    if shown == 0 {
        println!("  Nobody in this world was ever taken up, which is itself the finding.");
    }
}
