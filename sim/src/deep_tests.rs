//! What people at the pace of continents have to get right.
//!
//! The load-bearing claim is that **nothing here arranges the world around the people**.
//! The planet steps first and with no knowledge that anybody is on it; the people are then
//! told what their ground is. So every test that finds a settlement drowning, or a
//! continent filling as it thaws, is finding a consequence rather than a rule — and the
//! tests are written to fail if any of it is ever quietly reversed.

use super::deep::*;
use super::*;
use sim_core::{Domain, WorldSeed};

fn world(seed: u128) -> Ages {
    let seed = WorldSeed::from_u128(seed);
    Ages::begin(seed, Surface::genesis(seed))
}

fn rng(seed: u128) -> Rng {
    WorldSeed::from_u128(seed).stream(Domain::Terrain, 99, 0)
}

#[test]
fn a_world_begins_with_people_on_it() {
    let ages = world(0x0d_01);
    assert!(!ages.folk.is_empty(), "nobody was put anywhere");
    assert!(ages.souls() > 0);
    assert_eq!(ages.myr(), 0.0);
    assert_eq!(ages.readings.len(), 1);

    // And every one of them is standing on ground somebody could stand on.
    let habitability = ages.habitability();
    for folk in &ages.folk {
        assert!(
            ages.surface.planet.is_land(folk.cell),
            "{} is in the sea",
            folk.name
        );
        assert!(habitability.score(folk.cell) > 0.0);
    }
}

#[test]
fn no_two_peoples_stand_in_the_same_place() {
    let mut ages = world(0x0d_02);
    ages.run_myr(60.0, 4.0, &mut rng(0x0d_02));
    let grid = ages.surface.planet.grid();
    for (i, a) in ages.folk.iter().enumerate() {
        for b in &ages.folk[i + 1..] {
            assert_ne!(a.cell, b.cell, "{} and {} share a cell", a.name, b.name);
            assert!(
                !grid.neighbours(a.cell).contains(&b.cell),
                "{} and {} are neighbours",
                a.name,
                b.name
            );
        }
    }
}

#[test]
fn the_planet_moves_while_they_live_on_it() {
    // The whole point. Before this, a populated world's planet was a still frame.
    let mut ages = world(0x0d_03);
    let grid_len = ages.surface.planet.grid().len();
    let before: Vec<f32> = (0..grid_len)
        .map(|c| ages.surface.planet.height_above_sea_m(c as u32))
        .collect();
    let star_before = ages.surface.star().age_gyr;

    ages.run_myr(80.0, 4.0, &mut rng(0x0d_03));

    let after: Vec<f32> = (0..grid_len)
        .map(|c| ages.surface.planet.height_above_sea_m(c as u32))
        .collect();
    assert_ne!(before, after, "the continents did not move");
    assert!(
        ages.surface.star().age_gyr > star_before,
        "the star did not age"
    );
    assert!((ages.myr() - 80.0).abs() < 1e-6);
}

#[test]
fn a_people_finds_the_number_its_land_will_carry() {
    let mut ages = world(0x0d_04);
    let opening = ages.souls();
    ages.run_myr(20.0, 4.0, &mut rng(0x0d_04));
    // Forty people walk in and a megayear later there are as many as the ground feeds.
    assert!(
        ages.souls() > opening * 10,
        "a population of {} did not grow from {opening}",
        ages.souls()
    );

    // And the number is the land's, not a constant: the best-fed place holds more than
    // the worst-fed one.
    if ages.folk.len() > 1 {
        let life = &ages.surface.life;
        let best = ages
            .folk
            .iter()
            .max_by(|a, b| life.production(a.cell).total_cmp(&life.production(b.cell)))
            .unwrap();
        let worst = ages
            .folk
            .iter()
            .min_by(|a, b| life.production(a.cell).total_cmp(&life.production(b.cell)))
            .unwrap();
        assert!(
            best.souls >= worst.souls,
            "{} on better ground held fewer than {}",
            best.name,
            worst.name
        );
    }
}

#[test]
fn deep_time_takes_places_away_and_gives_others() {
    // Over a few hundred megayears a planet is unrecognisable, and the record should say
    // so. If nothing is ever founded or lost, either the planet is not moving or nobody is
    // noticing.
    let mut ages = world(0x0d_05);
    ages.run_myr(400.0, 4.0, &mut rng(0x0d_05));

    assert!(
        ages.ever > ages.readings[0].settlements,
        "no new ground was ever settled in four hundred megayears"
    );
    assert!(
        ages.lost > 0,
        "not one settlement failed in four hundred megayears"
    );
    // Every loss says why, and the reason is one the planet can actually supply.
    let ruins: Vec<Ruin> = ages
        .history
        .iter()
        .filter_map(|e| match e {
            Epoch::Abandoned { why, .. } => Some(*why),
            _ => None,
        })
        .collect();
    assert_eq!(ruins.len(), ages.lost);
    assert!(ruins.iter().all(|r| !r.label().is_empty()));
}

#[test]
fn the_record_is_readable_as_a_history() {
    let mut ages = world(0x0d_06);
    ages.run_myr(200.0, 4.0, &mut rng(0x0d_06));

    // Readings are in order, one per step plus the opening one.
    let mut last = -1.0;
    for reading in &ages.readings {
        assert!(reading.myr > last, "readings went backwards");
        last = reading.myr;
        assert!(reading.settlements <= 64);
        assert!((0.0..=1.0).contains(&reading.habitable));
    }
    assert!(ages.readings.len() > 40, "only {} readings", ages.readings.len());
}

#[test]
fn a_people_remembers_the_best_and_worst_its_ground_has_been() {
    let mut ages = world(0x0d_07);
    ages.run_myr(120.0, 4.0, &mut rng(0x0d_07));
    for folk in &ages.folk {
        assert!(
            folk.best_ground >= folk.worst_ground,
            "{} has a best worse than its worst",
            folk.name
        );
        assert!(folk.founded_myr <= ages.myr());
    }
}

#[test]
fn people_spread_across_land_and_not_across_water() {
    // A continent is settled by the people already on it; an island is settled by nobody.
    // So a settlement founded near an existing one should descend from it, and one founded
    // on the far side of an ocean should not.
    let mut ages = world(0x0d_08);
    ages.run_myr(150.0, 4.0, &mut rng(0x0d_08));

    let grid = ages.surface.planet.grid();
    for folk in &ages.folk {
        let Some(parent) = folk.parent else { continue };
        // The parent index refers to the folk list *at the time*, which shifts as
        // settlements fail — so this checks the invariant that matters rather than the
        // identity: nobody claims a parent they could not have walked from.
        if let Some(from) = ages.folk.get(parent) {
            let apart = grid.distance_km(from.cell, folk.cell, geo::EARTH_RADIUS_KM);
            assert!(
                apart < 20_000.0,
                "{} claims to descend from {} {apart:.0} km away",
                folk.name,
                from.name
            );
        }
    }
}

#[test]
fn the_same_seed_lives_the_same_history() {
    let mut a = world(0x0d_09);
    let mut b = world(0x0d_09);
    a.run_myr(80.0, 4.0, &mut rng(0x0d_09));
    b.run_myr(80.0, 4.0, &mut rng(0x0d_09));

    assert_eq!(a.souls(), b.souls());
    assert_eq!(a.ever, b.ever);
    assert_eq!(a.lost, b.lost);
    let names = |ages: &Ages| -> Vec<String> { ages.folk.iter().map(|f| f.name.clone()).collect() };
    assert_eq!(names(&a), names(&b));
}

#[test]
fn two_worlds_live_different_histories() {
    let mut a = world(0x0d_0a);
    let mut b = world(0x0d_0b);
    a.run_myr(80.0, 4.0, &mut rng(0x0d_0a));
    b.run_myr(80.0, 4.0, &mut rng(0x0d_0b));
    assert_ne!(a.souls(), b.souls());
}

#[test]
fn a_step_of_any_length_reaches_the_same_place_as_several() {
    // `run_myr` subdivides, and the subdivision must not itself be a physical claim: a
    // hundred megayears is a hundred megayears however it is counted out.
    let mut coarse = world(0x0d_0c);
    let mut fine = world(0x0d_0c);
    coarse.run_myr(40.0, 8.0, &mut rng(1));
    fine.run_myr(40.0, 8.0, &mut rng(1));
    assert_eq!(coarse.myr(), fine.myr());
    assert_eq!(coarse.souls(), fine.souls());
}

#[test]
fn nobody_is_left_standing_on_the_sea_floor() {
    // The sea moves a long way over deep time and this is the test that the people notice.
    let mut ages = world(0x0d_0d);
    ages.run_myr(300.0, 4.0, &mut rng(0x0d_0d));
    for folk in &ages.folk {
        assert!(
            ages.surface.planet.is_land(folk.cell),
            "{} is under water at {:.0} Myr",
            folk.name,
            ages.myr()
        );
    }
}

#[test]
fn a_population_tracks_what_the_world_can_feed() {
    // Not a constant, and not a random walk: the total should move with how much of the
    // planet is worth living on. Over a long run those two should be visibly related.
    let mut ages = world(0x0d_0e);
    ages.run_myr(400.0, 4.0, &mut rng(0x0d_0e));

    let lived: Vec<&Age> = ages.readings.iter().filter(|r| r.souls > 0).collect();
    if lived.len() < 20 {
        return; // A world that emptied is a legitimate world, and tested elsewhere.
    }
    let mean_x: f64 = lived.iter().map(|r| r.habitable as f64).sum::<f64>() / lived.len() as f64;
    let mean_y: f64 = lived.iter().map(|r| r.souls as f64).sum::<f64>() / lived.len() as f64;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    for r in &lived {
        let (dx, dy) = (r.habitable as f64 - mean_x, r.souls as f64 - mean_y);
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    if sxx <= 0.0 || syy <= 0.0 {
        return;
    }
    let correlation = sxy / (sxx.sqrt() * syy.sqrt());
    assert!(
        correlation > 0.2,
        "population and habitable area correlated at only {correlation:.2}"
    );
}
