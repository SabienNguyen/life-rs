//! Whether anybody in this world is in a position to take a side.
//!
//! Every act in §35 is one person to one person, and every consequence is too: the victim
//! holds a grudge, the actor holds a guilt, and up to three witnesses lower their opinion by
//! six hundredths. Nobody has ever done anything *together*, and nobody has ever turned on
//! somebody because of what was done to a third person. That is the largest remaining gap
//! between this and a society — feuds, factions, ostracism and collective punishment are all
//! the same shape, and none of them are reachable from a vocabulary that is strictly dyadic.
//!
//! The obvious mechanism is **partisanship**: what somebody makes of a wrong should depend on
//! who it was done *to*, not only on how bad it was. Hurting my brother is not the same event
//! as hurting a stranger, and a community where that is true splits along its existing ties
//! when somebody is wronged — which is a faction, arrived at without a faction ever being
//! declared.
//!
//! **This measures whether the world can supply that before any of it is built.** It needs a
//! third person who knows both parties, and it needs that third person to feel differently
//! about them — a C who is fond of both A and B has no side to take. Three times now this
//! project has built a correct mechanism behind a conjunction the world does not supply:
//! §32.2's conquest keyed on adjacent countries in a world with zero adjacent cross-country
//! pairs, §35's killing behind a gate past the end of the world, and §36.6's envy needing an
//! evening that happens five hundredths of one percent of the time. The pattern is always the
//! same and is always cheaper to check than to debug.
//!
//! It also asks whether collective feeling exists **already**. If some people are widely
//! disliked while others are widely liked, the raw material of ostracism is present and only
//! needs a mechanism to act on it. If dislike is scattered evenly, there is nothing to build
//! on and partisanship would be inventing the structure rather than using it.
//!
//!     cargo run --release --example who_takes_sides

use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

const SEEDS: [u128; 3] = [0x11, 0x21, 0x221];

/// Warmth past which somebody is a friend rather than an acquaintance, and past its negation
/// an enemy. Kept here rather than read from `bonds` so this says what it measured.
const FOND: f32 = 0.25;

fn main() {
    let years: u64 = std::env::var("YEARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(90);
    // §45.2's first claim is that a larger settlement outgrows universal acquaintance and
    // universal kinship on its own, without anything being added. That is falsifiable in one
    // run, so `FOUNDERS=n` makes it a measurement rather than a guess.
    let founders: usize = std::env::var("FOUNDERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);

    // **Founded once and shared.** This used to re-found every world in every section — nine
    // times over for three seeds, twenty-seven runs where three would do — which was tolerable
    // at a hundred and twenty founders and made §46.2's `FOUNDERS=600` protocol unusable. An
    // instrument too slow to point at the world you care about is not an instrument.
    let worlds: Vec<(u128, World)> = SEEDS
        .into_iter()
        .map(|seed| {
            let mut world = World::genesis(WorldSeed::from_u128(seed), founders);
            world.record_only(Salience::Pivotal);
            world.set_detail_budget(100_000);
            world.run_for(Duration::from_years(years));
            (seed, world)
        })
        .collect();

    // Triangles: for a tie that holds between A and B, how many third parties know them both.
    let (mut pairs, mut with_a_third) = (0usize, 0usize);
    let mut thirds_each: Vec<usize> = Vec::new();
    // And of those third parties, whether they have a side to take.
    let (mut sided, mut fond_of_both, mut cool_on_both) = (0usize, 0usize, 0usize);
    // Standing: how many people hold negative warmth toward each person, against how many
    // hold positive. The question is whether dislike concentrates on somebody or scatters.
    let mut disliked_by: Vec<(usize, usize, String)> = Vec::new();

    for (seed, world) in &worlds {

        let living: Vec<_> = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive() && p.has_matured())
            .map(|(id, p)| (id, p.name.clone()))
            .collect();

        // Who each person knows, once, so the triad walk is a set intersection rather than a
        // scan of the world per pair.
        let known: std::collections::BTreeMap<_, std::collections::BTreeSet<_>> = living
            .iter()
            .map(|(id, _)| {
                (
                    *id,
                    world
                        .bonds
                        .of(*id)
                        .filter(|(_, tie)| tie.holds())
                        .map(|(other, _)| other)
                        .collect(),
                )
            })
            .collect();

        for (a, _) in &living {
            let Some(a_knows) = known.get(a) else { continue };
            for b in a_knows {
                // Once per unordered pair. §39 measured this graph to be undirected in
                // everything but its type, so counting both ways would double everything and
                // say nothing.
                if b <= a {
                    continue;
                }
                let Some(b_knows) = known.get(b) else { continue };
                pairs += 1;
                let thirds: Vec<_> = a_knows.intersection(b_knows).copied().collect();
                thirds_each.push(thirds.len());
                if !thirds.is_empty() {
                    with_a_third += 1;
                }
                for c in thirds {
                    let to_a = world.bonds.tie(c, *a).warmth;
                    let to_b = world.bonds.tie(c, *b).warmth;
                    match (to_a > FOND, to_b > FOND) {
                        (true, true) => fond_of_both += 1,
                        (false, false) => cool_on_both += 1,
                        // Fond of exactly one of them — somebody with a side.
                        _ => sided += 1,
                    }
                }
            }
        }

        for (id, name) in &living {
            let (mut against, mut fory) = (0usize, 0usize);
            for (_, tie) in world.bonds.of(*id) {
                if !tie.holds() {
                    continue;
                }
                // Held *about* them is what a reputation is, but §39 found the graph
                // effectively undirected, so what they hold is the same reading and is the one
                // that is a lookup rather than a scan.
                if tie.warmth < -0.05 {
                    against += 1;
                } else if tie.warmth > FOND {
                    fory += 1;
                }
            }
            disliked_by.push((against, fory, name.clone()));
        }
    }

    thirds_each.sort_unstable();
    let at = |p: f32| thirds_each[((thirds_each.len() as f32 - 1.0) * p) as usize];
    let triads: usize = thirds_each.iter().sum();

    println!("{} seeds, {years} years\n", SEEDS.len());
    println!("  Can anybody take a side?\n");
    println!("  pairs who know each other        {pairs:>7}");
    println!(
        "  with at least one mutual friend  {with_a_third:>7}   ({:.1}% — the precondition)",
        100.0 * with_a_third as f32 / pairs.max(1) as f32
    );
    println!(
        "  third parties per pair           median {}, p90 {}, most {}",
        at(0.5),
        at(0.9),
        thirds_each.last().copied().unwrap_or(0)
    );
    println!("  (holder, subject, onlooker) triples  {triads:>7}\n");

    let share = |n: usize| 100.0 * n as f32 / triads.max(1) as f32;
    println!("  of those onlookers:");
    println!(
        "    fond of exactly one           {sided:>7}   ({:.1}%)  <- has a side to take",
        share(sided)
    );
    println!(
        "    fond of both                  {fond_of_both:>7}   ({:.1}%)  <- torn",
        share(fond_of_both)
    );
    println!(
        "    fond of neither               {cool_on_both:>7}   ({:.1}%)  <- indifferent",
        share(cool_on_both)
    );

    // And whether feeling concentrates. A society in which everybody is disliked by two
    // people has no outcasts; one in which a few are disliked by twenty has them already, and
    // ostracism would be a mechanism reading a structure rather than inventing one.
    disliked_by.sort_by_key(|(against, _, _)| std::cmp::Reverse(*against));
    let any = disliked_by.iter().filter(|(a, _, _)| *a > 0).count();
    // Is `regard` alive at all? `vitals` reports the spread of it about a person as 0.001,
    // which has two completely different explanations: everybody agrees because `hearsay` has
    // ground opinion flat, or everybody agrees because the number was never written and they
    // are all sitting at the zero it was born with. Those want opposite responses and the
    // difference is one count.
    println!("\n  Is `regard` alive?\n");
    let mut live = [(0usize, 0usize); 2];
    let mut sums = [0.0f64; 2];
    for (seed, world) in &worlds {
        for (who, person) in world.people.iter() {
            if !person.is_alive() {
                continue;
            }
            for (_, tie) in world.bonds.of(who) {
                if !tie.holds() {
                    continue;
                }
                for (at, value) in [(0usize, tie.regard), (1usize, tie.warmth)] {
                    live[at].1 += 1;
                    sums[at] += value.abs() as f64;
                    if value.abs() > 0.01 {
                        live[at].0 += 1;
                    }
                }
            }
        }
    }
    // And how much of that warmth is *hostile*, which is the material any avoidance mechanism
    // has to work with. `meet_repeatedly` pulls warmth toward `suits` on every one of 1.79
    // million evenings, and `suits` is temperament compatibility alone — so a dislike created
    // by an event is a displacement from an attractor, and the attractor wins. If almost no
    // live tie sits below `allied()`'s mirror of -0.25, then two people who get on will get on
    // whoever else they know, and a faction — which is precisely "I dislike you for who you
    // stand with" — has nothing to be built out of.
    {
        let (mut hostile, mut all) = (0usize, 0usize);
        for (seed, world) in &worlds {
            for (who, person) in world.people.iter() {
                if !person.is_alive() {
                    continue;
                }
                for (_, tie) in world.bonds.of(who) {
                    if !tie.holds() {
                        continue;
                    }
                    all += 1;
                    if tie.warmth < -0.25 {
                        hostile += 1;
                    }
                }
            }
        }
        println!(
            "  hostile ties (warmth < -0.25)  {hostile} of {all}  ({:.2}%)  <- what avoidance \
             has to work with",
            100.0 * hostile as f32 / all.max(1) as f32
        );
    }
    for (at, name) in [(0usize, "regard"), (1usize, "warmth")] {
        let (moved, all) = live[at];
        println!(
            "  {name:<7} moved off zero on {moved:>7} of {all:>7} live ties  ({:>5.1}%),  mean |value| {:.4}",
            100.0 * moved as f32 / all.max(1) as f32,
            sums[at] / all.max(1) as f64
        );
    }

    // Could a faction *hold*, though? Taking a side needs somebody to take it against, but a
    // faction needs a **boundary** — two groups who mostly talk within themselves — and
    // `hearsay` runs on every one of 1.79 million evenings dragging everybody's opinion toward
    // their friends'. Against a graph where everybody knows everybody, that is a consensus
    // engine with nothing to push back, and no partisanship mechanism built on top could
    // survive it. This is the §32.2 question asked about structure rather than about events.
    //
    // Label propagation: everybody starts in their own camp and repeatedly adopts whichever
    // camp their allies mostly belong to. On a graph with communities it settles into a few
    // large ones. On a graph without, it collapses to a single camp, which is the answer.
    //
    // Measured both ways on one build. Comparing against a remembered figure from a previous
    // build is what §42 spent a section learning not to do: the trajectories differ, so the
    // difference measures the divergence and not the mechanism.
    println!("\n  Could a faction hold?\n");
    for (seed, world) in &worlds {

        // **Within one place**, and that distinction is the whole question. Run over the
        // world, label propagation finds camps — and they are the settlements. People in
        // Ingwick befriend people in Ingwick, so 99% of friendships stay inside a camp and
        // the camps come out the size of the quarters. That is a map, not a faction.
        //
        // A faction is a split *inside* a community: two blocs in the same town who mostly
        // talk among themselves. So this asks the question of the biggest quarter alone,
        // where geography cannot answer it.
        let biggest_place = world
            .places
            .ids()
            .max_by_key(|p| world.society.households_in(*p).count());
        let folk: Vec<_> = world
            .people
            .iter()
            .filter(|(id, p)| {
                p.is_alive()
                    && p.has_matured()
                    && world.society.place_of(*id) == biggest_place
            })
            .map(|(id, _)| id)
            .collect();
        let at: std::collections::BTreeMap<_, usize> =
            folk.iter().enumerate().map(|(n, id)| (*id, n)).collect();
        let allies: Vec<Vec<usize>> = folk
            .iter()
            .map(|id| {
                world
                    .bonds
                    .of(*id)
                    .filter(|(_, t)| t.allied())
                    .filter_map(|(other, _)| at.get(&other).copied())
                    .collect()
            })
            .collect();
        let mut camp: Vec<usize> = (0..folk.len()).collect();
        for _ in 0..20 {
            let mut moved = false;
            for who in 0..folk.len() {
                let mut tally: std::collections::BTreeMap<usize, usize> = Default::default();
                for other in &allies[who] {
                    *tally.entry(camp[*other]).or_default() += 1;
                }
                if let Some((&best, _)) = tally.iter().max_by_key(|(_, n)| **n)
                    && best != camp[who]
                {
                    camp[who] = best;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        let mut sizes: std::collections::BTreeMap<usize, usize> = Default::default();
        for c in &camp {
            *sizes.entry(*c).or_default() += 1;
        }
        let mut counts: Vec<usize> = sizes.into_values().filter(|n| *n > 1).collect();
        counts.sort_unstable_by(|a, b| b.cmp(a));
        let biggest = counts.first().copied().unwrap_or(0);
        // How much of a person's social life stays inside their own camp. Anything near 1.0
        // with a single camp holding everybody is a world with no boundaries in it at all.
        let inside: usize = (0..folk.len())
            .map(|w| allies[w].iter().filter(|o| camp[**o] == camp[w]).count())
            .sum();
        let all: usize = allies.iter().map(Vec::len).sum();
        println!(
            "  seed {seed:x}: the biggest quarter's {} adults fall into {} camps of more than \
             one; biggest holds {biggest} ({:.0}%), and {:.0}% of friendships stay inside a camp",
            folk.len(),
            counts.len(),
            100.0 * biggest as f32 / folk.len().max(1) as f32,
            100.0 * inside as f32 / all.max(1) as f32
        );
    }

    // And how often the §43 term can speak at all. It fires when one of your *allies* holds
    // warmth below -0.25 toward somebody — an enemy of a friend. §36.6's rule, which this
    // project has now had to learn four times: count the occasions before diagnosing the
    // arithmetic. A term that fires on one relation in a thousand is inert whatever its weight.
    println!("\n  How often can a friend's enemy be avoided?\n");
    for (seed, world) in &worlds {

        let (mut through_ally, mut against, mut with) = (0usize, 0usize, 0usize);
        let mut anybody = 0usize;
        for (who, person) in world.people.iter() {
            if !person.is_alive() || !person.has_matured() {
                continue;
            }
            let mut has_one = false;
            for (friend, mine) in world.bonds.of(who) {
                if !mine.allied() {
                    continue;
                }
                for (_, theirs) in world.bonds.of(friend) {
                    if !theirs.holds() {
                        continue;
                    }
                    through_ally += 1;
                    if theirs.allied() {
                        with += 1;
                    } else if theirs.warmth < -0.25 {
                        against += 1;
                        has_one = true;
                    }
                }
            }
            if has_one {
                anybody += 1;
            }
        }
        println!(
            "  seed {seed:x}: of {through_ally} (me, my ally, someone they know) triples, \
             {with} are allies of an ally ({:.1}%) and {against} are enemies of one ({:.2}%); \
             {anybody} people have even one",
            100.0 * with as f32 / through_ally.max(1) as f32,
            100.0 * against as f32 / through_ally.max(1) as f32
        );
    }

    // §43.4's real finding, measured. If a town is one bloc because `choose_company` draws its
    // new faces from a **uniform** sample of the place, then friendships should show no trace
    // of the one axis the model already has to cluster on — what people do all day. Assortativity
    // by trade: how much likelier two friends are to share a trade than two people picked at
    // random from the same town. 1.00 means the uniform sample has dissolved it entirely.
    println!("\n  Do friendships cluster on anything at all?\n");
    for (seed, world) in &worlds {

        // Per place, so this measures clustering *within* a town rather than the fact that
        // towns differ in what they do — which is §43.1's mistake in miniature.
        let (mut same_tied, mut all_tied, mut same_pairs, mut all_pairs) = (0usize, 0usize, 0usize, 0usize);
        for place in world.places.ids() {
            let here: Vec<_> = world
                .people
                .iter()
                .filter(|(id, p)| {
                    p.is_alive() && p.has_matured() && world.society.place_of(*id) == Some(place)
                })
                .map(|(id, p)| (id, p.trade()))
                .collect();
            if here.len() < 10 {
                continue;
            }
            for (n, (a, trade_a)) in here.iter().enumerate() {
                for (b, trade_b) in here.iter().skip(n + 1) {
                    all_pairs += 1;
                    if trade_a == trade_b {
                        same_pairs += 1;
                    }
                    if world.bonds.tie(*a, *b).allied() {
                        all_tied += 1;
                        if trade_a == trade_b {
                            same_tied += 1;
                        }
                    }
                }
            }
        }
        let among_friends = same_tied as f32 / all_tied.max(1) as f32;
        let by_chance = same_pairs as f32 / all_pairs.max(1) as f32;
        println!(
            "  seed {seed:x}: trade — {:.1}% of friendships share one against {:.1}% of all pairs \
             in the same town ({:.2}x chance; base rate that high because most of a town farms)",
            100.0 * among_friends,
            100.0 * by_chance,
            among_friends / by_chance.max(1e-6)
        );

        // And kin, which is the one axis in this model that is genuinely partitioned — trade
        // cannot split a town when three quarters of it does the same thing. If friendships are
        // no likelier among kin either, then the uniform sample is dissolving *everything* and
        // the fix is not another term but the pool itself.
        let (mut kin_tied, mut tied, mut kin_pairs, mut pairs) = (0usize, 0usize, 0usize, 0usize);
        for place in world.places.ids() {
            let here: Vec<_> = world
                .people
                .iter()
                .filter(|(id, p)| {
                    p.is_alive() && p.has_matured() && world.society.place_of(*id) == Some(place)
                })
                .map(|(id, _)| id)
                .collect();
            if here.len() < 10 {
                continue;
            }
            for (n, a) in here.iter().enumerate() {
                for b in here.iter().skip(n + 1) {
                    let kin = world.society.is_close_kin(*a, *b);
                    pairs += 1;
                    kin_pairs += usize::from(kin);
                    if world.bonds.tie(*a, *b).allied() {
                        tied += 1;
                        kin_tied += usize::from(kin);
                    }
                }
            }
        }
        let kin_among = kin_tied as f32 / tied.max(1) as f32;
        let kin_chance = kin_pairs as f32 / pairs.max(1) as f32;
        println!(
            "           kin   — {:.1}% of friendships are close kin against {:.1}% of all pairs \
             ({:.2}x chance)",
            100.0 * kin_among,
            100.0 * kin_chance,
            kin_among / kin_chance.max(1e-6)
        );
    }

    // How many people does anybody actually know, against how many there are to know? A
    // two-step walk of the tie graph can only concentrate if two steps do not already reach
    // everybody. If a town of a hundred has people holding a hundred ties, the graph is
    // complete and *every* sampling scheme is a uniform one — which would make §44 marginal
    // for a reason that has nothing to do with how candidates are drawn.
    println!("\n  Is there room for structure?\n");
    for (seed, world) in &worlds {
        for place in world.places.ids() {
            let here: Vec<_> = world
                .people
                .iter()
                .filter(|(id, p)| {
                    p.is_alive() && p.has_matured() && world.society.place_of(*id) == Some(place)
                })
                .map(|(id, _)| id)
                .collect();
            if here.len() < 20 {
                continue;
            }
            let inside: Vec<usize> = here
                .iter()
                .map(|a| here.iter().filter(|b| *b != a && world.bonds.tie(*a, **b).holds()).count())
                .collect();
            let allied: Vec<usize> = here
                .iter()
                .map(|a| here.iter().filter(|b| world.bonds.tie(*a, **b).allied()).count())
                .collect();
            let mean = |v: &[usize]| v.iter().sum::<usize>() as f32 / v.len().max(1) as f32;
            println!(
                "  seed {seed:x} {:<14} {:>4} grown; each knows {:>5.1} of the other {:>3} \
                 ({:>4.0}% of the town) and is allied to {:.1}",
                world.places.get(place).map(|p| p.name.clone()).unwrap_or_default(),
                here.len(),
                mean(&inside),
                here.len() - 1,
                100.0 * mean(&inside) / (here.len() - 1).max(1) as f32,
                mean(&allied)
            );
        }
    }

    // §44 concluded that a complete acquaintance graph leaves no room for structure. But the
    // camps above were computed on `allied()`, not `holds()` — on a graph of 8 to 14 friends
    // out of 40 to 90 townspeople, 13% to 17% dense — and *that* graph still came back one
    // bloc. So completeness is not the whole story and this asks the sharper question: is the
    // friendship graph distinguishable from a random one of the same density?
    //
    // Clustering coefficient. In a random graph two of my friends are friends with each other
    // at the graph's own density; in a structured one, far more often. C/density near 1.0 means
    // friendship here is a coin flip weighted by nothing that groups anybody.
    println!("\n  Is friendship structured, or just sparse?\n");
    for (seed, world) in &worlds {
        for place in world.places.ids() {
            let here: Vec<_> = world
                .people
                .iter()
                .filter(|(id, p)| {
                    p.is_alive() && p.has_matured() && world.society.place_of(*id) == Some(place)
                })
                .map(|(id, _)| id)
                .collect();
            if here.len() < 30 {
                continue;
            }
            let allied: Vec<Vec<person::PersonId>> = here
                .iter()
                .map(|a| {
                    here.iter()
                        .copied()
                        .filter(|b| b != a && world.bonds.tie(*a, *b).allied())
                        .collect()
                })
                .collect();
            let edges: usize = allied.iter().map(Vec::len).sum();
            let density = edges as f32 / (here.len() * (here.len() - 1)) as f32;
            // How often two of somebody's friends are friends with each other.
            let (mut closed, mut wedges) = (0usize, 0usize);
            for friends in &allied {
                for (n, a) in friends.iter().enumerate() {
                    for b in friends.iter().skip(n + 1) {
                        wedges += 1;
                        if world.bonds.tie(*a, *b).allied() {
                            closed += 1;
                        }
                    }
                }
            }
            let clustering = closed as f32 / wedges.max(1) as f32;
            println!(
                "  seed {seed:x} {:<14} {:>3} grown, friendship density {:.3}, clustering {:.3} \
                 — {:.2}x a random graph",
                world.places.get(place).map(|p| p.name.clone()).unwrap_or_default(),
                here.len(),
                density,
                clustering,
                clustering / density.max(1e-6)
            );
        }
    }

    // §44.2 nominates kin as the discrete axis a faction could run on. Before building
    // anything on that: **is kin actually disjoint?** 12-18% of all pairs in a town are close
    // kin, which is a lot — if everybody is a cousin of everybody, kin is no more a partition
    // than similarity is, and would fail for exactly the reason §44.2 says similarity fails.
    //
    // This is the check whose absence caused §44's wrong conclusion, run before the mechanism
    // rather than after it.
    println!("\n  Is kin a partition, or is everyone a cousin?\n");
    for (seed, world) in &worlds {
        for place in world.places.ids() {
            let here: Vec<_> = world
                .people
                .iter()
                .filter(|(id, p)| {
                    p.is_alive() && p.has_matured() && world.society.place_of(*id) == Some(place)
                })
                .map(|(id, _)| id)
                .collect();
            if here.len() < 30 {
                continue;
            }
            // Connected components of the kin graph — the honest question, because a partition
            // is exactly a graph that falls apart into pieces.
            let mut group: Vec<usize> = (0..here.len()).collect();
            fn root(group: &mut Vec<usize>, mut n: usize) -> usize {
                while group[n] != n {
                    group[n] = group[group[n]];
                    n = group[n];
                }
                n
            }
            for (n, a) in here.iter().enumerate() {
                for (m, b) in here.iter().enumerate().skip(n + 1) {
                    if world.society.is_close_kin(*a, *b) {
                        let (ra, rb) = (root(&mut group, n), root(&mut group, m));
                        if ra != rb {
                            group[ra] = rb;
                        }
                    }
                }
            }
            let mut sizes: std::collections::BTreeMap<usize, usize> = Default::default();
            for n in 0..here.len() {
                *sizes.entry(root(&mut group, n)).or_default() += 1;
            }
            let mut counts: Vec<usize> = sizes.into_values().collect();
            counts.sort_unstable_by(|a, b| b.cmp(a));
            let biggest = counts.first().copied().unwrap_or(0);
            println!(
                "  seed {seed:x} {:<14} {:>3} grown fall into {:>3} kin groups; biggest holds \
                 {biggest} ({:.0}%), and {} are alone",
                world.places.get(place).map(|p| p.name.clone()).unwrap_or_default(),
                here.len(),
                counts.len(),
                100.0 * biggest as f32 / here.len() as f32,
                counts.iter().filter(|n| **n == 1).count()
            );
        }
    }

    println!("\n  Is anybody an outcast already?\n");
    println!(
        "  adults disliked by anybody at all  {any:>5} of {}   ({:.1}%)",
        disliked_by.len(),
        100.0 * any as f32 / disliked_by.len().max(1) as f32
    );
    println!("  the most disliked people in three worlds:");
    for (against, fory, name) in disliked_by.iter().take(8) {
        println!("    {name:<24} disliked by {against:>3}, liked by {fory:>3}");
    }
}
