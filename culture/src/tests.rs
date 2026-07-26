//! What an emergent culture has to get right.
//!
//! The load-bearing claim is that **nobody wrote a people down**. A world starts culturally
//! uniform; every people in it afterwards exists because places drifted apart, and every
//! name is built from what those places actually do. So the tests that matter are the ones
//! that would still pass if you had never heard of any of the peoples they produce —
//! isolation diverges, contact converges, small places drift faster than large ones.

use super::*;
use sim_core::{Domain, WorldSeed};

fn rng(seed: u128) -> Rng {
    WorldSeed::from_u128(seed).stream(Domain::Naming, 3, 0)
}

/// Everybody doing the ordinary thing.
fn ordinary(places: usize) -> Vec<[f32; WAYS]> {
    vec![[0.5; WAYS]; places]
}

#[test]
fn a_world_starts_as_one_people() {
    let world = Cultures::beginning(5, "Firstfolk");
    assert_eq!(world.len(), 1);
    for place in 0..5 {
        assert_eq!(world.of_place(place), 0);
    }
    assert_eq!(world.get(0).unwrap().parent, None);
}

#[test]
fn isolation_makes_a_people() {
    // The whole mechanism in one test. Five places, one of them out of contact with the
    // rest, left for long enough. Nothing tells it to become distinct; it drifts, and past
    // a distance it is a different people with a name and a parent.
    let mut world = Cultures::beginning(5, "Firstfolk");
    let mut rng = rng(0x15);
    let souls = [400u32, 400, 400, 400, 160];
    let contact = [0.9f32, 0.9, 0.9, 0.9, 0.0];

    for year in 0..600 {
        world.step(&ordinary(5), &contact, &souls, year, &mut rng);
    }

    assert!(
        world.len() > 1,
        "six hundred years of isolation produced no new people"
    );
    let stranded = world.of_place(4);
    assert_ne!(stranded, world.of_place(0), "the cut-off place stayed one of them");
    let people = world.get(stranded).unwrap();
    assert!(people.parent.is_some(), "a new people should know what it came from");
    assert!(!people.name.is_empty());
    assert_eq!(people.hearth, 4);
}

#[test]
fn contact_keeps_a_people_together() {
    // The other half. Same drift, same span, everybody in touch — and they stay one people.
    let mut world = Cultures::beginning(5, "Firstfolk");
    let mut rng = rng(0x16);
    let souls = [500u32; 5];
    let contact = [0.95f32; 5];

    for year in 0..600 {
        world.step(&ordinary(5), &contact, &souls, year, &mut rng);
    }

    let first = world.of_place(0);
    for place in 1..5 {
        assert_eq!(
            world.of_place(place),
            first,
            "places in constant contact split into different peoples"
        );
    }
}

#[test]
fn small_places_drift_faster_than_large_ones() {
    // The cultural analogue of genetic drift, and the same cause: fewer carriers, fewer
    // copies, more sampling error. It is why the distinctive peoples are the small ones.
    let mut world = Cultures::beginning(2, "Firstfolk");
    let mut rng = rng(0x17);
    let souls = [5_000u32, 20];
    let contact = [0.0f32, 0.0];

    for year in 0..300 {
        world.step(&ordinary(2), &contact, &souls, year, &mut rng);
    }

    let start = [0.5f32; WAYS];
    let big = distance(&world.practised(0), &start);
    let small = distance(&world.practised(1), &start);
    assert!(
        small > big * 1.5,
        "the small place drifted {small:.3} and a city {big:.3}"
    );
}

#[test]
fn what_people_do_pulls_hardest() {
    // A culture is a practice before it is an inheritance. If everybody in a place starts
    // working all the time, the place's ways should follow them.
    let mut world = Cultures::beginning(1, "Firstfolk");
    let mut rng = rng(0x18);
    let mut doing = [0.5f32; WAYS];
    doing[Deed::Work as usize] = 1.0;

    for year in 0..200 {
        world.step(&[doing], &[0.5], &[600], year, &mut rng);
    }
    assert!(
        world.practised(0)[Deed::Work as usize] > 0.85,
        "the place did not take up what its people were doing"
    );
}

#[test]
fn an_empty_place_keeps_its_manners() {
    // An emptied village does not forget how things were done; it simply has nobody doing
    // them. The same rule §14 already applies to a place's character.
    let mut world = Cultures::beginning(2, "Firstfolk");
    let mut rng = rng(0x19);
    let mut doing = [0.5f32; WAYS];
    doing[Deed::Wander as usize] = 0.95;
    for year in 0..100 {
        world.step(&[doing, doing], &[0.6, 0.6], &[400, 400], year, &mut rng);
    }
    let before = world.practised(1);

    for year in 100..400 {
        world.step(&[doing, doing], &[0.6, 0.6], &[400, 0], year, &mut rng);
    }
    assert_eq!(world.practised(1), before, "an empty place drifted");
}

#[test]
fn a_people_that_comes_back_is_the_people_it_was() {
    // Divergence and convergence have a gap between them on purpose. A place that drifts
    // out and back should return to the older culture rather than becoming a third thing,
    // or a world accumulates a name for every excursion.
    let mut world = Cultures::beginning(3, "Firstfolk");
    let mut rng = rng(0x1a);
    let apart = [0.9f32, 0.9, 0.0];
    let together = [0.9f32, 0.9, 0.95];
    let souls = [500u32, 500, 160];

    for year in 0..500 {
        world.step(&ordinary(3), &apart, &souls, year, &mut rng);
    }
    let wandered = world.of_place(2);

    for year in 500..1500 {
        world.step(&ordinary(3), &together, &souls, year, &mut rng);
    }
    let now = world.of_place(2);
    if now != wandered {
        // It came back to something. That something should be older than what it left.
        assert!(
            world.get(now).unwrap().arose <= world.get(wandered).unwrap().arose,
            "a returning people joined something younger than the one it left"
        );
    }
}

#[test]
fn a_country_is_a_culture_that_touches_itself() {
    let mut world = Cultures::beginning(4, "Firstfolk");
    let mut rng = rng(0x1b);
    let souls = [300u32, 300, 300, 40];
    // Three in a row that can reach each other, one cut off.
    let touching = |a: usize, b: usize| a < 3 && b < 3;

    for year in 0..600 {
        world.step(&ordinary(4), &[0.9, 0.9, 0.9, 0.0], &souls, year, &mut rng);
    }
    let countries = world.countries(&souls, touching);

    assert!(countries.len() >= 2, "the cut-off place is its own country");
    assert_eq!(countries[0].places.len(), 3, "the connected three are one");
    assert!(countries.iter().any(|c| c.places == vec![3]));
    // And every inhabited place is in exactly one.
    let mut counted: Vec<usize> = countries.iter().flat_map(|c| c.places.clone()).collect();
    counted.sort_unstable();
    assert_eq!(counted, vec![0, 1, 2, 3]);
}

#[test]
fn a_sea_between_two_halves_makes_two_countries() {
    // The same culture, no longer able to reach itself. This is the geography doing the
    // work rather than anybody deciding.
    let world = Cultures::beginning(4, "Firstfolk");
    let souls = [300u32; 4];
    let one = world.countries(&souls, |_, _| true);
    assert_eq!(one.len(), 1, "all in touch, all one country");

    let split = world.countries(&souls, |a, b| (a < 2) == (b < 2));
    assert_eq!(split.len(), 2, "a sea between them makes two");
    assert_eq!(split[0].culture, split[1].culture, "and they are still one people");
}

#[test]
fn nobody_lives_in_a_country_with_nobody_in_it() {
    let world = Cultures::beginning(4, "Firstfolk");
    let countries = world.countries(&[300, 0, 0, 120], |_, _| true);
    for country in &countries {
        assert!(!country.places.is_empty());
        for place in &country.places {
            assert!(*place == 0 || *place == 3, "an empty place joined a country");
        }
    }
}

#[test]
fn a_people_knows_who_it_descends_from() {
    let mut world = Cultures::beginning(3, "Firstfolk");
    let mut rng = rng(0x1c);
    let souls = [500u32, 160, 160];
    for year in 0..1500 {
        world.step(&ordinary(3), &[0.9, 0.0, 0.0], &souls, year, &mut rng);
    }
    for place in 0..3 {
        let culture = world.of_place(place);
        let line = world.ancestry(culture);
        // Every chain ends at an original, and no chain contains itself.
        assert!(!line.contains(&culture), "a people descends from itself");
        if let Some(oldest) = line.last() {
            assert_eq!(world.get(*oldest).unwrap().parent, None);
        }
    }
}

#[test]
fn new_places_join_the_people_who_walked_there() {
    let mut world = Cultures::beginning(2, "Firstfolk");
    let mut rng = rng(0x1d);
    for year in 0..400 {
        world.step(&ordinary(2), &[0.9, 0.0], &[500, 30], year, &mut rng);
    }
    let settlers = world.of_place(1);
    world.extend_to(3, Some(1));
    assert_eq!(world.of_place(2), settlers, "a new place appeared out of nowhere");
    assert_eq!(world.practised(2), world.practised(1));
}

#[test]
fn the_same_seed_grows_the_same_peoples() {
    let run = || {
        let mut world = Cultures::beginning(4, "Firstfolk");
        let mut r = rng(0x1e);
        for year in 0..400 {
            world.step(&ordinary(4), &[0.8, 0.4, 0.0, 0.9], &[300, 80, 20, 500], year, &mut r);
        }
        (0..4).map(|p| world.get(world.of_place(p)).unwrap().name.clone()).collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}

#[test]
fn a_lone_hamlet_becomes_one_people_and_then_stops() {
    // The other half of the fountain problem, and the one that only shows up once drift is
    // strong enough to work at all. An isolated hamlet crosses the threshold and takes a
    // name — and then keeps drifting, because drift does not stop. Measured against a
    // frozen record of the day it was named, it crosses again a few decades later, and
    // again, and one village produces a dozen peoples nobody ever practised.
    //
    // A culture is what its members currently do, so a place alone in its own culture is at
    // distance nought from it and cannot leave itself. One name, and then it is a people.
    let mut world = Cultures::beginning(2, "Firstfolk");
    let mut rng = rng(0x20);
    let souls = [900u32, 160];

    for year in 0..2_000 {
        world.step(&ordinary(2), &[0.8, 0.0], &souls, year, &mut rng);
    }

    assert!(world.len() >= 2, "two thousand years alone and still no people");
    assert_eq!(
        world.ever, 2,
        "one hamlet spawned {} peoples in two thousand years",
        world.ever
    );
    // And what it is now should be what it does now, not a record of some year in the past.
    let mine = world.of_place(1);
    assert!(
        distance(&world.get(mine).unwrap().ways, &world.practised(1)) < 0.001,
        "a people is not what its people do"
    );
}

#[test]
fn resembling_a_people_you_have_never_met_does_not_make_you_them() {
    // The rule that stopped the churn, stated on its own. A sealed place wanders, and
    // wandering in seven dimensions means it sometimes passes close to where the mainland
    // happens to be standing. Absorbing it at that moment is wrong twice over: it erases a
    // people on a coincidence, and it hands the place back so it can leave again, which is
    // what produced eight names for one village.
    //
    // The same distinction `evolution` draws. Two populations that look alike are not one
    // population; one population is two that actually meet.
    let mut world = Cultures::beginning(2, "Firstfolk");
    let mut rng = rng(0x22);
    let souls = [900u32, 160];

    // Long enough that the wandering really does pass close to the mainland at some point;
    // no need to contrive the coincidence, only to notice it.
    for year in 0..900 {
        world.step(&ordinary(2), &[0.8, 0.0], &souls, year, &mut rng);
    }
    let sealed = world.of_place(1);
    assert_ne!(sealed, world.of_place(0), "it never went its own way");

    let mut closest = f32::MAX;
    let mut still_itself = true;
    for year in 900..4_000 {
        world.step(&ordinary(2), &[0.8, 0.0], &souls, year, &mut rng);
        let mainland = world.get(world.of_place(0)).unwrap().ways;
        closest = closest.min(distance(&world.practised(1), &mainland));
        still_itself &= world.of_place(1) == sealed;
    }

    assert!(
        closest < THE_SAME_PEOPLE,
        "in three and a half millennia of wandering it never once came close ({closest:.3}) — \
         the test cannot say anything about what happens when it does"
    );
    assert!(
        still_itself,
        "a place nobody can reach was absorbed by a people it has never met"
    );

    // Now open a road. Same resemblance, but somebody actually arrives, and that is the
    // difference between looking alike and being one people.
    for year in 4_000..5_000 {
        world.step(&ordinary(2), &[0.8, 0.7], &souls, year, &mut rng);
    }
    assert_eq!(
        world.of_place(1),
        world.of_place(0),
        "they meet, they come to do the same things, and they are still two peoples"
    );
}

#[test]
fn a_people_nobody_practises_is_gone_but_remembered() {
    // Cultures are never removed from the list, because a people that died out is still
    // where its descendants came from. What it must not do is stay available to be rejoined
    // by a place that happens to drift past its last known ways.
    let mut world = Cultures::beginning(2, "Firstfolk");
    let mut rng = rng(0x21);
    for year in 0..1_200 {
        world.step(&ordinary(2), &[0.9, 0.0], &[600, 160], year, &mut rng);
    }
    let stranded = world.of_place(1);
    assert_ne!(stranded, world.of_place(0));

    // Everybody there dies. The people is gone; the record is not.
    for year in 1_200..1_400 {
        world.step(&ordinary(2), &[0.9, 0.0], &[600, 0], year, &mut rng);
    }
    assert!(!world.get(stranded).unwrap().living(), "an empty people has people");
    assert!(!world.get(stranded).unwrap().name.is_empty(), "it lost its name");
    assert!(world.get(world.of_place(0)).unwrap().living());
}

#[test]
fn living_differently_makes_a_people_faster_than_distance_does() {
    // Drift alone needs a small population and a long time. The commoner road to a distinct
    // people is that the place is *different*: mountain herders and river fishers do not
    // spend a day the same way, and after enough generations of not spending it the same
    // way they are not the same people.
    //
    // This matters for the world this crate goes into, where `doing` comes from what
    // terrain, food and work actually make people do. A place of six hundred would drift
    // apart from its neighbours in tens of thousands of years; living differently does it
    // in a few hundred.
    let mut world = Cultures::beginning(2, "Firstfolk");
    let mut rng = rng(0x23);
    // A lowland and a smaller upland behind it, both far too big for drift to do anything
    // in six centuries, in the thin contact a mountain road affords.
    let souls = [1_200u32, 400];

    let settled = [0.5f32; WAYS];
    let mut roving = [0.5f32; WAYS];
    roving[Deed::Wander as usize] = 0.95;
    roving[Deed::Work as usize] = 0.90;
    roving[Deed::Sleep as usize] = 0.15;

    for year in 0..600 {
        world.step(&[settled, roving], &[0.2, 0.2], &souls, year, &mut rng);
    }

    assert_ne!(
        world.of_place(0),
        world.of_place(1),
        "six centuries of living completely differently and they are still one people"
    );
    // And the name they took should be about the thing that made them different.
    let theirs = &world.get(world.of_place(1)).unwrap().name;
    assert!(
        theirs.starts_with("Roam") || theirs.starts_with("Unquiet") || theirs.starts_with("Toil"),
        "a roving people who never sleep were named {theirs}"
    );
}

#[test]
fn a_band_is_not_a_people_however_far_it_drifts() {
    // Sampling error at twenty carriers is enormous, so a quarter of twenty people rattles
    // right across the space of ways in a decade. That is real and the model should keep
    // it. What it must not do is call the result a nation.
    //
    // The world this went into is made of neighbourhoods of two to forty souls, and without
    // a floor it named three peoples inside three years — one per quarter, each of them a
    // handful of families whose habits had wandered. A people has to be big enough that a
    // practice survives being passed to somebody you do not personally know.
    let mut world = Cultures::beginning(2, "Firstfolk");
    let mut rng = rng(0x24);
    let band = [900u32, 20];

    for year in 0..3_000 {
        world.step(&ordinary(2), &[0.8, 0.0], &band, year, &mut rng);
    }

    assert_eq!(world.len(), 1, "twenty people became a nation");
    assert_eq!(world.of_place(1), world.of_place(0));
    // And it really did drift — this is a floor on being named, not on changing.
    assert!(
        distance(&world.practised(1), &world.practised(0)) > A_DIFFERENT_PEOPLE,
        "the fixture did not actually drift the band anywhere"
    );

    // Grow it past the floor and the same drift now makes it a people.
    let grown = [900u32, 400];
    for year in 3_000..3_100 {
        world.step(&ordinary(2), &[0.8, 0.0], &grown, year, &mut rng);
    }
    assert_ne!(
        world.of_place(1),
        world.of_place(0),
        "a band that grew into a nation was still not allowed to be one"
    );
}

#[test]
fn a_world_does_not_shatter_into_a_people_per_village() {
    // The fountain problem `evolution` hit with speciation, in its cultural form. Every
    // place drifts a little; a threshold that catches ordinary variation would name a new
    // people every century per village.
    let mut world = Cultures::beginning(8, "Firstfolk");
    let mut rng = rng(0x1f);
    let souls = [200u32; 8];
    let contact = [0.6f32; 8];
    for year in 0..800 {
        world.step(&ordinary(8), &contact, &souls, year, &mut rng);
    }
    let distinct: std::collections::BTreeSet<usize> =
        (0..8).map(|p| world.of_place(p)).collect();
    assert!(
        distinct.len() <= 4,
        "eight villages in contact became {} peoples",
        distinct.len()
    );
}
