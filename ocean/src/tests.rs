//! What a sea has to get right.
//!
//! The load-bearing test here is `the_fisheries_are_on_the_eastern_shores`. Nothing in
//! this crate knows the word "eastern" — the wind belts are a function of latitude and
//! the Ekman rule is a rotation — so if the great upwelling zones come out on the eastern
//! sides of basins, they came out of the physics rather than out of a table. That is the
//! same standard the rest of the project holds itself to and it is the only reason to
//! prefer this over painting fisheries onto a map.

use super::*;
use sim_core::{Domain, WorldSeed};

const SEED: u128 = 0x0cea_0117;

fn planet() -> (Lithosphere, Climate) {
    let seed = WorldSeed::from_u128(SEED);
    let mut rng = seed.stream(Domain::Terrain, 0, 0);
    let mut planet = Lithosphere::genesis(4, 9, 0.40, &mut rng);
    planet.step_myr(4.0, &mut rng);
    let climate = Climate::genesis(&planet, 4.5, climate::insolation::EARTH_OBLIQUITY);
    (planet, climate)
}

#[test]
fn the_wind_belts_are_where_the_wind_belts_are() {
    // Easterly trades, westerlies, polar easterlies — the three-cell circulation, which
    // is not a choice so much as a consequence of how much sun falls where.
    assert!(wind_at(10.0).eastward < 0.0, "the trades blow from the east");
    assert!(wind_at(-10.0).eastward < 0.0);
    assert!(wind_at(45.0).eastward > 0.0, "the westerlies blow from the west");
    assert!(wind_at(-45.0).eastward > 0.0);
    assert!(wind_at(75.0).eastward < 0.0, "the polar easterlies");

    // And the trades blow towards the equator in both hemispheres, which is what makes
    // them converge there.
    assert!(wind_at(15.0).poleward < 0.0);
    assert!(wind_at(-15.0).poleward < 0.0);
}

#[test]
fn the_wind_is_a_mirror_across_the_equator() {
    for latitude in [5.0, 22.0, 40.0, 58.0, 75.0] {
        let north = wind_at(latitude);
        let south = wind_at(-latitude);
        assert_eq!(north.eastward, south.eastward, "at {latitude}°");
        assert_eq!(north.poleward, south.poleward, "at {latitude}°");
    }
}

#[test]
fn nothing_upwells_on_dry_land() {
    let (planet, climate) = planet();
    let sea = Ocean::read(&planet, &climate);
    for cell in planet.grid().cells() {
        if planet.is_land(cell) {
            assert_eq!(sea.upwelling(cell), 0.0, "cell {cell} is a field");
            assert_eq!(sea.nutrients(cell), 0.0);
            assert_eq!(sea.basin(cell), NO_BASIN);
        } else {
            assert!(sea.basin(cell) != NO_BASIN, "cell {cell} is water with no sea");
            assert!(sea.upwelling(cell) > 0.0, "a dead calm everywhere");
        }
    }
}

#[test]
fn the_equator_is_a_line_of_rising_water() {
    // Equatorial divergence: the Coriolis parameter changes sign, so the trades push
    // surface water away from the line in both directions. The Pacific cold tongue.
    let (planet, climate) = planet();
    let sea = Ocean::read(&planet, &climate);
    let grid = planet.grid();

    let mean_between = |low: f64, high: f64| {
        let mut total = 0.0;
        let mut count = 0;
        for cell in grid.cells() {
            if planet.is_land(cell) {
                continue;
            }
            let latitude = grid.position(cell).latitude().to_degrees().abs();
            if (low..high).contains(&latitude) {
                total += sea.upwelling(cell) as f64;
                count += 1;
            }
        }
        if count == 0 { 0.0 } else { total / count as f64 }
    };

    let equatorial = mean_between(0.0, 4.0);
    let subtropical = mean_between(14.0, 24.0);
    assert!(equatorial > 0.0 && subtropical > 0.0, "the planet needs both bands");
    assert!(
        equatorial > subtropical * 1.5,
        "the equator upwelled {equatorial:.3} against {subtropical:.3} in the subtropics"
    );
}

#[test]
fn the_fisheries_are_on_the_eastern_shores() {
    // The test this crate exists to pass. Nothing here knows the word "eastern": the wind
    // is a function of latitude and Ekman transport is a ninety-degree rotation. If the
    // strong coastal upwelling nevertheless lands on the eastern sides of basins — Peru,
    // California, Benguela, the Canaries — it came out of the geometry.
    //
    // "Eastern side of a basin" means: water with land immediately to its east.
    let (planet, climate) = planet();
    let sea = Ocean::read(&planet, &climate);
    let grid = planet.grid();

    let mut eastern = (0.0f64, 0usize);
    let mut western = (0.0f64, 0usize);
    for cell in grid.cells() {
        if planet.is_land(cell) {
            continue;
        }
        let latitude = grid.position(cell).latitude().to_degrees();
        // The trade-wind belt, and only it. The equator has a different mechanism that
        // would drown the signal, and past the horse latitudes the westerlies reverse the
        // sense — which is also true of the real planet, and is why the great eastern
        // boundary currents are subtropical rather than temperate.
        if !(12.0..HORSE_LATITUDE).contains(&latitude.abs()) {
            continue;
        }
        let here = grid.position(cell);
        let east = geo::Vec3::new(0.0, 0.0, 1.0).cross(here);
        if east.length() < 1e-6 {
            continue;
        }
        let east = east.normalised();

        for &n in grid.neighbours(cell) {
            if !planet.is_land(n) {
                continue;
            }
            let toward = grid.position(n).minus(here).normalised();
            let side = toward.dot(east);
            // Land clearly to the east, or clearly to the west; ignore land due north
            // or south, which tells us nothing about which shore of a basin this is.
            if side > 0.5 {
                eastern.0 += sea.upwelling(cell) as f64;
                eastern.1 += 1;
            } else if side < -0.5 {
                western.0 += sea.upwelling(cell) as f64;
                western.1 += 1;
            }
            break;
        }
    }

    assert!(
        eastern.1 > 5 && western.1 > 5,
        "not enough coast to compare: {} eastern, {} western",
        eastern.1,
        western.1
    );
    let east_mean = eastern.0 / eastern.1 as f64;
    let west_mean = western.0 / western.1 as f64;
    assert!(
        east_mean > west_mean,
        "upwelling came out stronger on the western shores ({west_mean:.3}) than the \
         eastern ({east_mean:.3}) — the Ekman rotation has the wrong sign"
    );
}

#[test]
fn the_rotation_flips_across_the_equator_and_so_does_the_wind() {
    // Two reversals that cancel, which is the whole reason Peru and California are both
    // upwelling coasts despite being mirror images of each other.
    //
    // The rotation itself genuinely reverses — check it on the same physical wind, which
    // means the same *northward* component rather than the same hemisphere-relative one.
    let due_north = Wind { eastward: 0.0, poleward: 1.0 };
    let due_south = Wind { eastward: 0.0, poleward: -1.0 };
    let (north_side, _) = ekman_transport(due_north, 25.0);
    // In the south, poleward is southward, so a truly northward wind is poleward = −1.
    let (south_side, _) = ekman_transport(due_south, -25.0);
    assert!(
        north_side * south_side < 0.0,
        "the same wind deflected the same way in both hemispheres: \
         {north_side} and {south_side}"
    );

    // And now the cancellation. The real trades, at mirrored latitudes: both push the
    // surface layer *westward*, so both leave an eastern shore short of water.
    let (north_offshore, _) = ekman_transport(wind_at(20.0), 20.0);
    let (south_offshore, _) = ekman_transport(wind_at(-20.0), -20.0);
    assert!(
        north_offshore < 0.0 && south_offshore < 0.0,
        "the trades did not drive water west in both hemispheres: \
         {north_offshore} and {south_offshore}"
    );
    assert!((north_offshore - south_offshore).abs() < 1e-6);
}

#[test]
fn a_warm_ocean_turns_over_more_slowly() {
    // The overturning is driven by water cold enough to sink. A planet with no cold water
    // has a sluggish ocean, which is what the hothouse intervals in the record look like.
    let (planet, cold) = planet();
    let brisk = Ocean::read(&planet, &cold);

    // The same planet far later in its star's life, where the sun is half again as
    // bright. The thermostat damps that and does not cancel it.
    let warm = Climate::genesis(&planet, 9.0, climate::insolation::EARTH_OBLIQUITY);
    let sluggish = Ocean::read(&planet, &warm);

    assert!(
        warm.mean_temperature_c(&planet) > cold.mean_temperature_c(&planet),
        "the late-life planet was not hotter"
    );
    assert!(
        sluggish.mean_upwelling(&planet) < brisk.mean_upwelling(&planet),
        "a hothouse ocean overturned faster than a cold one: {:.3} against {:.3}",
        sluggish.mean_upwelling(&planet),
        brisk.mean_upwelling(&planet)
    );
}

#[test]
fn the_tropical_open_ocean_is_a_desert_and_the_shelf_is_not() {
    // The single most visible fact about ocean colour from orbit, and the reason this
    // crate exists: light and warmth are not what the sea is short of.
    let (planet, climate) = planet();
    let sea = Ocean::read(&planet, &climate);
    let grid = planet.grid();

    let mut open = (0.0f64, 0usize);
    let mut shelf = (0.0f64, 0usize);
    for cell in grid.cells() {
        if planet.is_land(cell) {
            continue;
        }
        let latitude = grid.position(cell).latitude().to_degrees().abs();
        if !(10.0..28.0).contains(&latitude) {
            continue;
        }
        let bucket = if nutrients::is_shelf(&planet, cell) {
            &mut shelf
        } else {
            &mut open
        };
        bucket.0 += sea.nutrients(cell) as f64;
        bucket.1 += 1;
    }
    assert!(open.1 > 3 && shelf.1 > 3, "not enough subtropical sea to compare");
    assert!(
        shelf.0 / shelf.1 as f64 > open.0 / open.1 as f64,
        "the subtropical shelf was no better fed than the open ocean"
    );
}

#[test]
fn cold_water_is_better_fed_than_warm_water() {
    // Winter mixing. The high-latitude ocean has a spring bloom and the tropics do not,
    // because cold surface water is dense and a winter storm turns the column over.
    let (planet, climate) = planet();
    let sea = Ocean::read(&planet, &climate);
    let grid = planet.grid();

    let mut cold = (0.0f64, 0usize);
    let mut warm = (0.0f64, 0usize);
    for cell in grid.cells() {
        if planet.is_land(cell) || nutrients::is_shelf(&planet, cell) {
            continue;
        }
        let bucket = if climate.temperature_c(cell) < 6.0 {
            &mut cold
        } else if climate.temperature_c(cell) > 22.0 {
            &mut warm
        } else {
            continue;
        };
        bucket.0 += sea.nutrients(cell) as f64;
        bucket.1 += 1;
    }
    if cold.1 < 3 || warm.1 < 3 {
        return; // A planet without both is not a failure of the model.
    }
    assert!(
        cold.0 / cold.1 as f64 > warm.0 / warm.1 as f64,
        "the warm open ocean was better fed than the cold"
    );
}

#[test]
fn every_drop_of_water_belongs_to_exactly_one_sea() {
    let (planet, climate) = planet();
    let sea = Ocean::read(&planet, &climate);
    assert!(sea.basins() >= 1, "a planet with oceans has at least one");

    // A basin is connected: every wet neighbour of a wet cell is in the same sea.
    let grid = planet.grid();
    for cell in grid.cells() {
        if planet.is_land(cell) {
            continue;
        }
        for &n in grid.neighbours(cell) {
            if !planet.is_land(n) {
                assert_eq!(
                    sea.basin(cell),
                    sea.basin(n),
                    "cells {cell} and {n} touch and are in different seas"
                );
            }
        }
    }
}

#[test]
fn some_of_the_sea_is_worth_fishing_and_not_all_of_it() {
    let (planet, climate) = planet();
    let sea = Ocean::read(&planet, &climate);
    let share = sea.fertile_share(&planet);
    assert!(
        (0.02..0.8).contains(&share),
        "{:.0}% of the sea came out fertile",
        share * 100.0
    );
}

#[test]
fn the_same_planet_reads_the_same_sea() {
    let (planet, climate) = planet();
    let first = Ocean::read(&planet, &climate);
    let second = Ocean::read(&planet, &climate);
    for cell in planet.grid().cells() {
        assert_eq!(first.upwelling(cell), second.upwelling(cell));
        assert_eq!(first.nutrients(cell), second.nutrients(cell));
    }
}
