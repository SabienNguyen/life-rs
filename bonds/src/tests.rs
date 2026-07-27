//! What a society of ties has to get right.
//!
//! The claim under all of it is that **nobody wrote a friendship down**. Every tie here
//! exists because two people spent time together, and every faction exists because enough
//! of those ties point the same way. So the tests that matter are the ones that would still
//! pass if you had never heard of any particular pair — meeting warms, neglect fades, an
//! unpaid debt sours, an opinion travels along a warm tie and stops at a cold one.

use super::*;
use circles::{FEWEST_TO_STAND, circles};
use sim_core::{Arena, Domain, WorldSeed};

/// A handful of real people to hang ties between.
///
/// Real rather than bare handles, because `suits` is part of what is being tested and it
/// needs temperaments to compare. The arena outlives the test by being leaked, which is
/// what a test may do and a simulation may not.
fn ids(how_many: usize) -> Vec<PersonId> {
    folk(how_many).0
}

/// The same, keeping the arena, for the tests that need whole people rather than handles.
fn folk(how_many: usize) -> (Vec<PersonId>, &'static Arena<person::Person>) {
    folk_with(how_many, &|_| 0.0)
}

/// The same, with means, for the tests where what somebody has is part of the question.
fn folk_with(
    how_many: usize,
    standing: &dyn Fn(usize) -> f32,
) -> (Vec<PersonId>, &'static Arena<person::Person>) {
    let arena: &'static mut Arena<person::Person> = Box::leak(Box::new(Arena::new()));
    let pool = genetics::FounderPool::uniform();
    let mut home: Arena<planet::Planet> = Arena::new();
    let world = home.insert(planet::Planet::earth());
    let who: Vec<PersonId> = (0..how_many)
        .map(|i| {
            let mut r = WorldSeed::from_u128(0x9000 + i as u128)
                .stream(Domain::Genetics, i as u64, 0);
            arena.insert(person::found(
                genetics::standard_architecture(),
                &pool,
                &mut r,
                world,
                sim_core::Time::ORIGIN,
                0.0,
            ))
        })
        .collect();
    for (at, id) in who.iter().enumerate() {
        if let Some(person) = arena.get_mut(*id) {
            person.set_standing(standing(at));
        }
    }
    (who, arena)
}

fn rng(seed: u128) -> Rng {
    WorldSeed::from_u128(seed).stream(Domain::Behavior, 7, 0)
}

fn always_alive(_: PersonId) -> bool {
    true
}

#[test]
fn strangers_hold_nothing() {
    let who = ids(2);
    let bonds = Bonds::new();
    assert_eq!(bonds.tie(who[0], who[1]), Tie::STRANGERS);
    assert!(!bonds.tie(who[0], who[1]).holds());
    assert_eq!(bonds.len(), 0);
}

#[test]
fn meeting_makes_a_tie_and_it_runs_both_ways() {
    let who = ids(2);
    let mut bonds = Bonds::new();
    bonds.meet(who[0], who[1], 0.9);
    assert!(bonds.tie(who[0], who[1]).holds());
    assert!(bonds.tie(who[1], who[0]).holds(), "a meeting has two people in it");
}

#[test]
fn people_who_suit_each_other_grow_warm_and_people_who_do_not_grow_cold() {
    let who = ids(4);
    let mut bonds = Bonds::new();
    for _ in 0..30 {
        bonds.meet(who[0], who[1], 0.95);   // alike
        bonds.meet(who[2], who[3], 0.05);   // unalike
    }
    assert!(bonds.tie(who[0], who[1]).warmth > 0.4, "thirty evenings and still not friends");
    assert!(bonds.tie(who[2], who[3]).warmth < -0.2, "thirty evenings and no worse");
}

#[test]
fn a_tie_nobody_tends_goes() {
    // Moving away has to cost something, or a place cannot be tight and nowhere can be
    // left behind.
    let who = ids(2);
    let mut bonds = Bonds::new();
    for _ in 0..20 {
        bonds.meet(who[0], who[1], 0.9);
    }
    let close = bonds.tie(who[0], who[1]).known;
    for _ in 0..14 {
        bonds.year(&always_alive);
    }
    assert!(bonds.tie(who[0], who[1]).known < close);
    assert_eq!(bonds.len(), 0, "a tie nobody has tended in fourteen years is not faint, it is gone");
}

#[test]
fn help_given_is_help_owed() {
    let who = ids(2);
    let mut bonds = Bonds::new();
    bonds.helped(who[0], who[1], 3.0);
    assert_eq!(bonds.tie(who[0], who[1]).debt, 3.0, "the giver is owed");
    assert_eq!(bonds.tie(who[1], who[0]).debt, -3.0, "the taker owes");
    assert!(bonds.tie(who[1], who[0]).warmth > 0.0, "being helped is warming");
}

#[test]
fn a_debt_that_goes_unpaid_sours_the_one_who_is_owed() {
    // The whole of reciprocity in one test. Nothing tells the creditor to resent anybody;
    // it falls out of being out of pocket for long enough.
    let who = ids(2);
    let mut bonds = Bonds::new();
    for _ in 0..12 {
        bonds.meet(who[0], who[1], 0.9);
    }
    let fond = bonds.tie(who[0], who[1]).warmth;
    bonds.helped(who[0], who[1], 30.0);
    for _ in 0..6 {
        bonds.year(&always_alive);
    }
    assert!(
        bonds.tie(who[0], who[1]).warmth < fond,
        "thirty days given, none returned, and no hard feelings"
    );
}

#[test]
fn paying_up_is_what_saves_it() {
    let who = ids(2);
    let mut bonds = Bonds::new();
    for _ in 0..12 {
        bonds.meet(who[0], who[1], 0.9);
    }
    bonds.helped(who[0], who[1], 20.0);
    bonds.repaid(who[1], who[0], 20.0);
    let settled = bonds.tie(who[0], who[1]);
    assert_eq!(settled.debt, 0.0);
    assert!(settled.regard > 0.0, "paying up should count for something");

    let mut sour = Bonds::new();
    for _ in 0..12 {
        sour.meet(who[0], who[1], 0.9);
    }
    sour.helped(who[0], who[1], 20.0);
    for _ in 0..5 {
        sour.year(&always_alive);
    }
    let mut kept = bonds;
    for _ in 0..5 {
        kept.year(&always_alive);
    }
    assert!(
        kept.tie(who[0], who[1]).warmth > sour.tie(who[0], who[1]).warmth,
        "the debtor who paid is no better liked than the one who did not"
    );
}

#[test]
fn an_opinion_travels_along_a_warm_tie() {
    // Gossip, with no words in it. A knows B, B thinks poorly of C, and A comes to think
    // a little poorly of C without ever having met them.
    let who = ids(3);
    let (a, b, c) = (who[0], who[1], who[2]);
    let mut bonds = Bonds::new();
    for _ in 0..20 {
        bonds.meet(a, b, 0.95);
        bonds.meet(b, c, 0.05);
    }
    // B's low opinion of C, made explicit.
    for _ in 0..8 {
        bonds.helped(b, c, 4.0);
    }
    for _ in 0..4 {
        bonds.year(&always_alive);
    }
    let before = bonds.tie(a, c).regard;
    for _ in 0..20 {
        bonds.hearsay(a, b);
    }
    assert!(
        bonds.tie(a, c).regard < before,
        "A spent twenty evenings with B and learned nothing about C"
    );
}

#[test]
fn an_opinion_stops_at_a_cold_tie() {
    // The other half, and the reason this is reputation rather than broadcast: you do not
    // take the word of somebody you dislike.
    let who = ids(3);
    let (a, b, c) = (who[0], who[1], who[2]);
    let mut bonds = Bonds::new();
    for _ in 0..25 {
        bonds.meet(a, b, 0.02);   // they cannot stand each other
        bonds.meet(b, c, 0.95);
    }
    let before = bonds.tie(a, c).regard;
    for _ in 0..25 {
        bonds.hearsay(a, b);
    }
    assert_eq!(
        bonds.tie(a, c).regard,
        before,
        "an opinion crossed a tie that should not carry one"
    );
}

#[test]
fn people_who_stand_together_are_a_circle_and_two_people_are_not() {
    let who = ids(5);
    let mut bonds = Bonds::new();
    // Three who all get on, and a pair off to one side.
    for _ in 0..25 {
        for a in 0..3 {
            for b in 0..3 {
                if a != b {
                    bonds.meet(who[a], who[b], 0.95);
                }
            }
        }
        bonds.meet(who[3], who[4], 0.95);
    }
    let found = circles(&bonds, &who);
    assert_eq!(found.len(), 1, "a pair is a friendship, not a faction");
    assert_eq!(found[0].members.len(), FEWEST_TO_STAND);
    assert!(found[0].cohesion > 0.0);
}

#[test]
fn a_circle_needs_the_warmth_to_run_both_ways() {
    // Directed ties are the whole reason for the shape of this: a hanger-on is not in the
    // circle he would like to be in, and a model where liking is always mutual cannot say
    // so.
    let who = ids(3);
    let mut bonds = Bonds::new();
    for _ in 0..25 {
        bonds.meet(who[0], who[1], 0.95);
        bonds.meet(who[1], who[2], 0.95);
        bonds.meet(who[0], who[2], 0.95);
    }
    // The third of them is quietly cut by both.
    for holder in [who[0], who[1]] {
        for _ in 0..40 {
            bonds.helped(holder, who[2], 5.0);
        }
    }
    for _ in 0..8 {
        bonds.year(&always_alive);
    }
    let found = circles(&bonds, &who);
    for circle in &found {
        assert!(
            !circle.members.contains(&who[2]),
            "somebody everybody resents is standing in their circle"
        );
    }
}

#[test]
fn allies_lend_their_weight() {
    let who = ids(4);
    let mut bonds = Bonds::new();
    for _ in 0..25 {
        bonds.meet(who[0], who[1], 0.95);
        bonds.meet(who[0], who[2], 0.95);
    }
    // The two being compared are equally poor; everybody else is comfortable.
    let standing = |id: PersonId| Some(if id == who[0] || id == who[3] { 0.2 } else { 0.9 });
    let backed = standing_with_allies(&bonds, who[0], 0.2, &standing);
    let alone = standing_with_allies(&bonds, who[3], 0.2, &standing);
    assert!(
        backed > alone,
        "a poor man with rich friends weighed no more than one with none: {backed} vs {alone}"
    );
    assert!(backed < 0.2 + 0.9 + 0.9, "an ally is not the same as being them");
}

#[test]
fn the_dead_are_let_go() {
    let who = ids(3);
    let mut bonds = Bonds::new();
    for _ in 0..20 {
        bonds.meet(who[0], who[1], 0.9);
        bonds.meet(who[0], who[2], 0.9);
    }
    bonds.forget(who[1]);
    assert!(!bonds.tie(who[0], who[1]).holds());
    assert!(!bonds.tie(who[1], who[0]).holds());
    assert!(bonds.tie(who[0], who[2]).holds(), "letting one go took another with it");
}

#[test]
fn company_is_chosen_and_strangers_are_still_met() {
    let who = ids(6);
    let mut bonds = Bonds::new();
    for _ in 0..25 {
        bonds.meet(who[0], who[1], 0.95);
    }
    let mut r = rng(0x50);
    let mut counts = std::collections::BTreeMap::new();
    for _ in 0..400 {
        if let Some(picked) = bonds.choose_company(who[0], &who, &mut r) {
            *counts.entry(picked).or_insert(0) += 1;
        }
    }
    let friend = counts.get(&who[1]).copied().unwrap_or(0);
    let stranger = counts.get(&who[4]).copied().unwrap_or(0);
    assert!(friend > stranger, "a friend was no likelier company than a stranger");
    assert!(
        stranger > 0,
        "nobody ever met anybody new, so no tie could ever start"
    );
}

#[test]
fn a_season_in_one_call_is_the_season() {
    // What the coarse tier stands on. If these came apart, who your friends are would
    // depend on who the observer happened to be looking at.
    let who = ids(2);
    let (mut slow, mut fast) = (Bonds::new(), Bonds::new());
    for _ in 0..40 {
        slow.meet(who[0], who[1], 0.8);
    }
    fast.meet_repeatedly(who[0], who[1], 0.8, 40);
    let (a, b) = (slow.tie(who[0], who[1]), fast.tie(who[0], who[1]));
    assert!((a.known - b.known).abs() < 1e-5, "{a:?} against {b:?}");
    assert!((a.warmth - b.warmth).abs() < 1e-5, "{a:?} against {b:?}");
}

#[test]
fn a_season_of_talk_is_the_talk() {
    let who = ids(3);
    let (a, b, c) = (who[0], who[1], who[2]);
    let mut slow = Bonds::new();
    let mut fast = Bonds::new();
    for (into, batched) in [(&mut slow, false), (&mut fast, true)] {
        for _ in 0..20 {
            into.meet(a, b, 0.95);
            into.meet(b, c, 0.9);
        }
        for _ in 0..6 {
            into.repaid(c, b, 5.0);
        }
        if batched {
            into.hearsay_repeatedly(a, b, 30);
        } else {
            for _ in 0..30 {
                into.hearsay(a, b);
            }
        }
    }
    let (one, many) = (slow.tie(a, c).regard, fast.tie(a, c).regard);
    assert!(one > 0.0, "nothing was passed on at all");
    assert!(
        (one - many).abs() < 0.02,
        "thirty conversations one at a time gave {one:.3}, all at once gave {many:.3}"
    );
}

#[test]
fn hearing_of_somebody_is_not_knowing_them() {
    // Otherwise a decade of ordinary gossip leaves everybody in a town as familiar to each
    // other as lifelong friends, and the cost of it grows with the square of the population.
    let who = ids(3);
    let (a, b, c) = (who[0], who[1], who[2]);
    let mut bonds = Bonds::new();
    for _ in 0..25 {
        bonds.meet(a, b, 0.95);
        bonds.meet(b, c, 0.95);
    }
    bonds.hearsay_repeatedly(a, b, 5_000);
    let heard = bonds.tie(a, c);
    assert!(heard.holds(), "A never even heard of C");
    assert!(
        !heard.allied(),
        "A has never met C and counts them an ally: {heard:?}"
    );
    assert!(heard.known <= 0.26, "known {} by reputation alone", heard.known);
}

#[test]
fn a_circle_is_everybody_with_everybody_and_not_a_chain() {
    // The reading that a connected component gave: two triangles joined by one friendship
    // came back as one faction of six. Mutual affection does not divide a society into
    // camps, because liking is not transitive but reachability is.
    let who = ids(6);
    let mut bonds = Bonds::new();
    for _ in 0..30 {
        for group in [[0, 1, 2], [3, 4, 5]] {
            for a in group {
                for b in group {
                    if a != b {
                        bonds.meet(who[a], who[b], 0.95);
                    }
                }
            }
        }
        // The one link between them.
        bonds.meet(who[2], who[3], 0.95);
    }
    let found = circles(&bonds, &who);
    assert!(found.len() >= 2, "two circles joined by a friendship read as {found:?}");
    assert!(
        found.iter().all(|c| c.members.len() <= 3),
        "a chain of friendships came back as one faction: {found:?}"
    );
}

#[test]
fn the_same_seed_makes_the_same_evening() {
    let who = ids(5);
    let bonds = Bonds::new();
    let run = || {
        let mut r = rng(0x51);
        (0..30)
            .filter_map(|_| bonds.choose_company(who[0], &who, &mut r))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}

// ---- positions in a society (§26) ---------------------------------------------------

use roles::{Facts, Role, among};

/// A person with a life behind them, so a position can be read off it.
fn a_life(who: PersonId, arena: &'static Arena<person::Person>) -> &'static person::Person {
    arena.get(who).expect("just inserted")
}

#[test]
fn everybody_alike_is_everybody_ordinary() {
    // The floor under the whole reading. A society with no differences in it has no
    // positions in it, and must not invent any.
    let (who, arena) = folk(6);
    let bonds = Bonds::new();
    let facts: Vec<Facts> = who
        .iter()
        .map(|w| Facts { who: *w, person: a_life(*w, arena), age: 30.0 })
        .collect();
    for (_, _, role) in among(&bonds, &facts) {
        assert_eq!(role, Role::Householder, "a position appeared where nobody differs");
    }
}

#[test]
fn nobody_is_a_patron_if_nobody_owes_them_anything() {
    // Proximity in a space of measurements is not a relation. Somebody can have more of
    // everything than anybody and still not be a patron, because a patron is defined by
    // somebody else owing them and that either happened or it did not.
    let (who, arena) = folk(5);
    let mut bonds = Bonds::new();
    for other in &who[1..] {
        for _ in 0..25 {
            bonds.meet(who[0], *other, 0.95);
        }
    }
    let facts: Vec<Facts> = who
        .iter()
        .enumerate()
        .map(|(at, w)| Facts {
            who: *w,
            person: a_life(*w, arena),
            age: if at == 0 { 70.0 } else { 30.0 },
        })
        .collect();
    let read = among(&bonds, &facts);
    let (_, _, role) = read.iter().find(|(id, _, _)| *id == who[0]).expect("read");
    assert_ne!(*role, Role::Patron, "a patron with no clients");
    assert_ne!(*role, Role::Elder, "an elder nobody owes anything to");
}

#[test]
fn the_one_everybody_owes_is_read_as_a_patron_and_the_one_carried_most_as_a_client() {
    let (who, arena) = folk(8);
    let mut bonds = Bonds::new();
    for other in &who[1..] {
        for _ in 0..25 {
            bonds.meet(who[0], *other, 0.95);
        }
        // A little for most of them, and a great deal for one.
        bonds.helped(who[0], *other, if *other == who[1] { 400.0 } else { 20.0 });
    }
    let facts: Vec<Facts> = who
        .iter()
        .map(|w| Facts { who: *w, person: a_life(*w, arena), age: 40.0 })
        .collect();
    let read = among(&bonds, &facts);
    let of = |w| read.iter().find(|(id, _, _)| *id == w).map(|(_, _, r)| *r);
    assert_eq!(of(who[0]), Some(Role::Patron), "the one who carried everybody: {read:?}");
    assert_eq!(of(who[1]), Some(Role::Client), "the one carried most: {read:?}");
}

#[test]
fn age_is_what_separates_an_elder_from_a_patron() {
    // The same relation, one lifetime apart. Nothing else about the two differs.
    let (who, arena) = folk_with(8, &|at| if at < 2 { 0.8 } else { 0.3 });
    let mut bonds = Bonds::new();
    for other in &who[2..] {
        for holder in [who[0], who[1]] {
            for _ in 0..25 {
                bonds.meet(holder, *other, 0.95);
            }
            bonds.helped(holder, *other, 40.0);
        }
    }
    let facts: Vec<Facts> = who
        .iter()
        .enumerate()
        .map(|(at, w)| Facts {
            who: *w,
            person: a_life(*w, arena),
            // Spread, so that being thirty-four is unremarkable rather than seventh of
            // eight. A rank is only as meaningful as the spread it is taken over.
            age: match at {
                0 => 78.0,
                1 => 34.0,
                _ => 24.0 + at as f64 * 6.0,
            },
        })
        .collect();
    let read = among(&bonds, &facts);
    let of = |w| read.iter().find(|(id, _, _)| *id == w).map(|(_, _, r)| *r);
    assert_eq!(of(who[0]), Some(Role::Elder));
    assert_eq!(of(who[1]), Some(Role::Patron));
}

#[test]
fn a_position_is_held_against_the_neighbours_and_not_against_a_number() {
    // A rich man in a poor village is the patron; the same man among richer neighbours is
    // nobody in particular. No threshold written anywhere could say that, which is why
    // every quantity here is a rank.
    let (who, arena) = folk(6);
    let mut bonds = Bonds::new();
    for other in &who[1..] {
        for _ in 0..25 {
            bonds.meet(who[0], *other, 0.95);
        }
        bonds.helped(who[0], *other, 40.0);
    }
    let facts = |ages: &[f64]| -> Vec<Facts> {
        who.iter()
            .enumerate()
            .map(|(at, w)| Facts { who: *w, person: a_life(*w, arena), age: ages[at] })
            .collect()
    };
    let among_equals = among(&bonds, &facts(&[40.0; 6]));
    // Now put the same person among people who all carry each other just as much.
    let mut even = Bonds::new();
    for a in &who {
        for b in &who {
            if a != b {
                for _ in 0..25 {
                    even.meet(*a, *b, 0.95);
                }
                even.helped(*a, *b, 40.0);
            }
        }
    }
    let among_peers = among(&even, &facts(&[40.0; 6]));
    let role_of = |read: &[(PersonId, roles::Position, Role)]| {
        read.iter().find(|(id, _, _)| *id == who[0]).map(|(_, _, r)| *r)
    };
    assert_eq!(role_of(&among_equals), Some(Role::Patron));
    assert_ne!(
        role_of(&among_peers),
        Some(Role::Patron),
        "everybody carrying everybody equally should make nobody a patron"
    );
}

#[test]
fn somebody_widely_thought_poorly_of_is_shunned() {
    // The only sanction in this world. Nobody decides it and nowhere is it written down:
    // it is what being carried and not making it good comes to, once opinion has travelled.
    let (who, arena) = folk(7);
    let mut bonds = Bonds::new();
    for other in &who[1..] {
        for _ in 0..25 {
            bonds.meet(*other, who[0], 0.5);
        }
        // Everybody carried them, and they never made it good.
        bonds.helped(*other, who[0], 60.0);
    }
    for _ in 0..12 {
        bonds.year(&always_alive);
    }
    assert!(
        bonds.repute_of(who[0]) < 0.0,
        "carried by everybody and repaid nobody, and nobody thinks the worse of them"
    );
    let facts: Vec<Facts> = who
        .iter()
        .map(|w| Facts { who: *w, person: a_life(*w, arena), age: 35.0 })
        .collect();
    let read = among(&bonds, &facts);
    let (_, _, role) = read.iter().find(|(id, _, _)| *id == who[0]).expect("read");
    assert!(
        matches!(role, Role::Outcast | Role::Client),
        "the one everybody carried and nobody rates is {role:?}"
    );
}

#[test]
fn a_reputation_is_what_others_hold_and_cannot_be_read_off_your_own_ties() {
    let who = ids(3);
    let mut bonds = Bonds::new();
    for _ in 0..20 {
        bonds.meet(who[0], who[1], 0.9);
        bonds.meet(who[0], who[2], 0.9);
    }
    bonds.helped(who[1], who[0], 30.0);
    bonds.repaid(who[0], who[1], 30.0);
    assert!(bonds.repute_of(who[0]) > 0.0, "paying up counted for nothing");
    // And the walk of the whole graph agrees with the walk of one person's.
    let all = bonds.everybodys_repute();
    let (total, holders) = all.get(&who[0]).copied().unwrap_or((0.0, 0));
    assert!(holders > 0);
    assert!((total / holders as f32 - bonds.repute_of(who[0])).abs() < 1e-5);
}
