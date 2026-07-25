//! What a planet has to do to be believable.
//!
//! Most of these are not unit tests. They are claims about emergent behaviour over
//! hundreds of megayears — that ocean floor recycles, that mountains rise where plates
//! meet and come down when they stop, that the largest landmass both assembles and
//! breaks up — and every one of them would be trivially satisfiable by a script. That is
//! the point of testing them: they hold here because nothing draws them.

use super::*;
use sim_core::{Domain, WorldSeed};

fn rng_for(seed: u128) -> Rng {
    WorldSeed::from_u128(seed).stream(Domain::Terrain, 0, 0)
}

/// A planet at the coarsest resolution that still behaves.
///
/// Level three is cheaper and does not work: with cells nine hundred kilometres across,
/// plates weld faster than they rift, the whole surface ends up as one plate, and the
/// supercontinent never breaks up again. Level four is where the cycle turns.
///
/// Four tenths continental crust, which is Earth's — a good deal of it submerged shelf
/// rather than dry land, also as Earth's is.
fn a_planet(seed: u128) -> (Lithosphere, Rng) {
    let mut rng = rng_for(seed);
    let planet = Lithosphere::genesis(4, 9, 0.42, &mut rng);
    (planet, rng)
}

fn run(planet: &mut Lithosphere, rng: &mut Rng, myr: u32, dt: f32) {
    for _ in 0..(myr as f32 / dt) as u32 {
        planet.step_myr(dt, rng);
    }
}

// ---- the grid the planet is built on --------------------------------------------

#[test]
fn a_new_planet_is_covered_exactly_once() {
    let (planet, _) = a_planet(0x1);
    for cell in planet.grid().cells() {
        let plate = planet.plate_of(cell);
        assert!(
            planet.plates()[plate as usize].active,
            "cell {cell} belongs to a plate that is not there"
        );
        assert!(planet.thickness_km(cell) > 0.0);
        assert!(planet.elevation_m(cell).is_finite());
    }
}

#[test]
fn the_requested_share_of_the_surface_is_continent() {
    for wanted in [0.15f32, 0.30, 0.55] {
        let mut rng = rng_for(0x2);
        let planet = Lithosphere::genesis(4, 8, wanted, &mut rng);
        let got = planet.continental_fraction();
        assert!(
            (got - wanted).abs() < 0.04,
            "asked for {wanted} continent and got {got}"
        );
    }
}

#[test]
fn an_earthlike_planet_starts_earthlike() {
    // Nothing sets the shoreline. The crust is where isostasy puts it, the water fills
    // what is left, and roughly a quarter to a third of the surface comes out dry — with
    // the dry part standing a few hundred metres to a couple of kilometres above the
    // waterline, which is what a planet looks like.
    let (planet, _) = a_planet(0x3);
    let land = planet.land_fraction();
    assert!(
        (0.15..0.40).contains(&land),
        "a fresh planet was {land} dry land"
    );

    let dry: Vec<CellId> = planet
        .grid()
        .cells()
        .filter(|c| planet.is_land(*c))
        .collect();
    let mean: f32 = dry
        .iter()
        .map(|c| planet.height_above_sea_m(*c))
        .sum::<f32>()
        / dry.len() as f32;
    assert!(
        (200.0..2500.0).contains(&mean),
        "the land averaged {mean:.0} m above the sea"
    );
    // And the sea has to be below the ordinary continent it is lapping against, or the
    // datum has come adrift from the crust it is defined by.
    assert!(
        planet.sea_level_m() < 0.0,
        "sea level came out above unstretched continental crust at {} m",
        planet.sea_level_m()
    );
}

#[test]
fn a_planet_with_no_continents_is_all_ocean() {
    let mut rng = rng_for(0x4);
    let planet = Lithosphere::genesis(4, 6, 0.0, &mut rng);
    assert_eq!(planet.continental_fraction(), 0.0);
    assert!(
        planet.land_fraction() < 0.02,
        "a waterworld had {} dry",
        planet.land_fraction()
    );
}

// ---- determinism ----------------------------------------------------------------

#[test]
fn the_same_seed_builds_the_same_planet() {
    let build = || {
        let (mut planet, mut rng) = a_planet(0xABC);
        run(&mut planet, &mut rng, 60, 2.0);
        planet
            .grid()
            .cells()
            .map(|c| (planet.elevation_m(c), planet.plate_of(c)))
            .collect::<Vec<_>>()
    };
    assert_eq!(build(), build());
}

#[test]
fn a_different_seed_builds_a_different_planet() {
    // The whole point of a seeded world: a new one is genuinely new. Two planets
    // agreeing to within a metre everywhere would mean the seed was not reaching the
    // terrain.
    let elevations = |seed| {
        let (mut planet, mut rng) = a_planet(seed);
        run(&mut planet, &mut rng, 40, 2.0);
        planet
            .grid()
            .cells()
            .map(|c| planet.elevation_m(c))
            .collect::<Vec<f32>>()
    };
    let (a, b) = (elevations(0x11), elevations(0x22));
    let same = a
        .iter()
        .zip(&b)
        .filter(|(x, y)| (*x - *y).abs() < 1.0)
        .count();
    assert!(
        same < a.len() / 4,
        "two seeds agreed on {same} of {} cells",
        a.len()
    );
}

// ---- the conveyor ----------------------------------------------------------------

#[test]
fn ocean_floor_is_young_and_continents_are_old() {
    // The single most striking fact about the real sea floor: none of it is older than
    // about a fifth of a billion years, while the continents go back four. It is a
    // consequence of subduction, not something anyone has to arrange.
    let (mut planet, mut rng) = a_planet(0x5);
    run(&mut planet, &mut rng, 400, 2.0);

    let oldest_ocean = planet
        .grid()
        .cells()
        .filter(|c| planet.crust(*c).is_oceanic())
        .map(|c| planet.crust_age_myr(c))
        .fold(0.0f32, f32::max);
    let oldest_land = planet
        .grid()
        .cells()
        .filter(|c| !planet.crust(*c).is_oceanic())
        .map(|c| planet.crust_age_myr(c))
        .fold(0.0f32, f32::max);

    assert!(
        oldest_land > oldest_ocean,
        "continent {oldest_land} Myr, ocean {oldest_ocean} Myr"
    );
}

#[test]
fn the_sea_floor_is_not_the_one_the_planet_started_with() {
    // The conveyor, stated as the fact it is on the real planet: no ocean floor is very
    // old, because all of it eventually reaches a trench. Measured as a median rather
    // than a snapshot count — spreading comes in bursts as the supercontinent cycle
    // turns, so any single instant can catch the planet in a quiet phase and say
    // nothing about whether the conveyor is running.
    let (mut planet, mut rng) = a_planet(0x6);
    run(&mut planet, &mut rng, 600, 2.0);

    let mut ages: Vec<f32> = planet
        .grid()
        .cells()
        .filter(|c| planet.crust(*c).is_oceanic())
        .map(|c| planet.crust_age_myr(c))
        .collect();
    assert!(!ages.is_empty(), "the planet has no ocean");
    ages.sort_by(f32::total_cmp);
    let median = ages[ages.len() / 2];
    assert!(
        median < 350.0,
        "half the sea floor is older than {median:.0} Myr, on a planet 600 Myr into its \
         run — nothing is being recycled"
    );
}

// ---- mountains -------------------------------------------------------------------

#[test]
fn plates_meeting_build_mountains() {
    // Nothing in the code raises terrain. Collisions thicken crust, and isostasy does
    // the rest — so if there are no highlands after a few hundred megayears, the
    // mechanism is not connected.
    let (mut planet, mut rng) = a_planet(0x7);
    let before = highest(&planet);
    run(&mut planet, &mut rng, 300, 2.0);
    let after = highest(&planet);
    assert!(
        after > before + 500.0,
        "the highest point went from {before:.0} m to {after:.0} m"
    );
    assert!(
        after < 12_000.0,
        "and it should not reach {after:.0} m — crust founders long before that"
    );
}

#[test]
fn mountains_come_down_when_nothing_holds_them_up() {
    // Erosion against isostatic rebound, on a real planet's topography but with the
    // tectonics taken out of the loop entirely — this drives the two steps directly
    // rather than through `step_myr`.
    //
    // Trying to arrange a quiet planet instead does not work, and it is worth saying
    // why: stopping the plates does not stop them, because a reorganisation hands out a
    // fresh rate; putting everything on one plate does not help either, because a plate
    // holding all of a planet's continental crust is exactly the plate that rifts. The
    // machinery is built so that a still planet is not one of its states.
    let mut rng = rng_for(0x8);
    let mut planet = Lithosphere::genesis(4, 6, 0.42, &mut rng);
    let crest = planet.grid.position(0);
    let mut range = Vec::new();
    for cell in 0..planet.grid.len() {
        let away = planet.grid.position(cell as CellId).angle_to(crest);
        if planet.crust[cell] == CrustType::Continental && away < 0.35 {
            planet.thickness_km[cell] = 62.0;
            range.push(cell as CellId);
        }
    }
    assert!(
        range.len() > 8,
        "only {} cells of range to watch",
        range.len()
    );
    planet.settle();

    // Averaged across the range, and measured against the datum.
    //
    // Across, because the summit is a drainage divide: almost no water crosses it and
    // it is the very last thing to go, so watching the highest point watches the one
    // cell that erodes least — which is why the first version of this test recorded no
    // change at all over four hundred megayears.
    //
    // Against the datum, because with nothing renewing the ocean floor it all cools and
    // sinks together, and a sea a kilometre lower makes every mountain look taller while
    // the mountain does nothing of the kind.
    let summit = |planet: &Lithosphere| {
        range
            .iter()
            .map(|c| planet.elevation_m(*c) as f64)
            .sum::<f64>()
            / range.len() as f64
    };
    let before = summit(&planet);
    for _ in 0..200 {
        planet.wear_down(2.0);
        planet.settle();
    }
    let after = summit(&planet);
    assert!(
        after < before - 300.0,
        "in 400 Myr of rain the range averaged {after:.0} m, from {before:.0} m"
    );
    assert!(
        after > 200.0,
        "the range was planed to {after:.0} m, which is far too fast"
    );
}

fn highest(planet: &Lithosphere) -> f32 {
    planet
        .grid()
        .cells()
        .map(|c| planet.height_above_sea_m(c))
        .fold(f32::MIN, f32::max)
}

#[test]
fn what_erodes_off_the_land_arrives_in_the_basins() {
    let (mut planet, mut rng) = a_planet(0x9);
    run(&mut planet, &mut rng, 100, 2.0);
    let laid: f32 = planet.grid().cells().map(|c| planet.sediment_m(c)).sum();
    assert!(laid > 0.0, "a hundred megayears of rivers moved nothing");

    let underwater: f32 = planet
        .grid()
        .cells()
        .filter(|c| !planet.is_land(*c))
        .map(|c| planet.sediment_m(c))
        .sum();
    assert!(
        underwater > laid * 0.5,
        "only {underwater:.0} m of {laid:.0} m of sediment reached the sea"
    );
}

// ---- the supercontinent cycle -----------------------------------------------------

#[test]
fn continents_gather_and_break_apart_again() {
    // The claim the whole design rests on: the supercontinent cycle is not scripted. It
    // is what plates that weld on collision and rift when they get too big do. Watched
    // over a billion years, the largest landmass has to both grow and shrink — a planet
    // that only aggregates has no rifting, one that only disperses has no welding, and
    // either way half the mechanism is dead.
    let (mut planet, mut rng) = a_planet(0xC0FFEE);
    let mut share = Vec::new();
    for _ in 0..250 {
        planet.step_myr(4.0, &mut rng);
        share.push(planet.largest_landmass_share());
    }

    let peak = share.iter().copied().fold(0.0f32, f32::max);
    let trough_after_peak = {
        let at = share.iter().position(|s| *s == peak).unwrap();
        share[at..].iter().copied().fold(1.0f32, f32::min)
    };
    assert!(
        peak > 0.55,
        "the continents never gathered: the largest mass peaked at {peak:.2}"
    );
    assert!(
        trough_after_peak < peak - 0.12,
        "having gathered to {peak:.2} they never broke up, bottoming at {trough_after_peak:.2}"
    );
}

#[test]
fn plates_both_weld_and_rift() {
    let (mut planet, mut rng) = a_planet(0xBEEF);
    let mut counts = Vec::new();
    for _ in 0..250 {
        planet.step_myr(4.0, &mut rng);
        counts.push(planet.active_plates());
    }
    let fell = counts.windows(2).any(|w| w[1] < w[0]);
    let rose = counts.windows(2).any(|w| w[1] > w[0]);
    assert!(fell, "no two plates ever welded: {counts:?}");
    assert!(rose, "no plate ever rifted: {counts:?}");
}

#[test]
fn every_kind_of_boundary_occurs() {
    let (mut planet, mut rng) = a_planet(0xD);
    run(&mut planet, &mut rng, 100, 2.0);
    let mut kinds = std::collections::BTreeSet::new();
    for cell in planet.grid().cells() {
        kinds.insert(planet.boundary(cell));
    }
    for wanted in [
        Boundary::Interior,
        Boundary::Divergent,
        Boundary::Convergent,
        Boundary::Transform,
    ] {
        assert!(
            kinds.contains(&wanted),
            "no {} boundary anywhere",
            wanted.label()
        );
    }
}

// ---- the sea ---------------------------------------------------------------------

#[test]
fn sea_level_moves_with_the_shape_of_the_basins() {
    // Not a constant, and not forced. The same water in changing basins stands at
    // different heights, which is where transgressions and regressions come from.
    let (mut planet, mut rng) = a_planet(0xE);
    let mut levels = Vec::new();
    for _ in 0..120 {
        planet.step_myr(4.0, &mut rng);
        levels.push(planet.sea_level_m());
    }
    let low = levels.iter().copied().fold(f32::MAX, f32::min);
    let high = levels.iter().copied().fold(f32::MIN, f32::max);
    assert!(
        high - low > 30.0,
        "sea level never moved: {low:.0} m to {high:.0} m"
    );
    assert!(
        high - low < 4000.0,
        "sea level swung {:.0} m, which is not a planet",
        high - low
    );
}

#[test]
fn a_planet_neither_drowns_nor_dries_out() {
    // The stability claim. Half a billion years with no drift into a waterworld or a
    // desert, which is the failure mode of a model where creation and destruction of
    // crust are not in balance.
    let (mut planet, mut rng) = a_planet(0xF);
    for step in 0..250 {
        planet.step_myr(2.0, &mut rng);
        let land = planet.land_fraction();
        assert!(
            (0.03..0.75).contains(&land),
            "after {} Myr the planet was {land} dry",
            step * 2
        );
    }
}

// ---- numerical health --------------------------------------------------------------

#[test]
fn nothing_goes_to_nonsense_over_deep_time() {
    let (mut planet, mut rng) = a_planet(0x1234);
    run(&mut planet, &mut rng, 600, 2.0);
    for cell in planet.grid().cells() {
        let h = planet.elevation_m(cell);
        assert!(h.is_finite(), "cell {cell} elevation went to {h}");
        assert!(
            (-12_000.0..12_000.0).contains(&h),
            "cell {cell} reached {h} m"
        );
        assert!(planet.thickness_km(cell) > 0.0);
        assert!(planet.thickness_km(cell) <= MAX_CRUST_KM + 0.01);
        assert!(planet.sediment_m(cell) >= 0.0);
        assert!(planet.crust_age_myr(cell) >= 0.0);
    }
    assert!(planet.sea_level_m().is_finite());
    assert!(
        planet.active_plates() >= 2,
        "the planet lost all its plates"
    );
}

#[test]
fn crust_is_not_quietly_created_or_destroyed_in_bulk() {
    // Subduction and spreading should roughly balance. A drift of a few percent over
    // half a billion years is the model working; a doubling is a leak.
    let volume = |planet: &Lithosphere| -> f64 {
        planet
            .grid()
            .cells()
            .map(|c| planet.thickness_km(c) as f64 * planet.area_km2[c as usize])
            .sum()
    };
    let (mut planet, mut rng) = a_planet(0x77);
    let before = volume(&planet);
    run(&mut planet, &mut rng, 500, 2.0);
    let after = volume(&planet);
    let change = (after - before).abs() / before;
    assert!(change < 0.5, "crust volume moved by {:.0}%", change * 100.0);
}

#[test]
fn a_long_step_is_survivable_even_though_it_is_coarse() {
    // Deep time will want to stride. It should degrade, not explode.
    let (mut planet, mut rng) = a_planet(0x88);
    for _ in 0..20 {
        planet.step_myr(50.0, &mut rng);
    }
    assert!(
        planet
            .grid()
            .cells()
            .all(|c| planet.elevation_m(c).is_finite())
    );
    assert!(planet.land_fraction() > 0.0);
}

// ---- addressing --------------------------------------------------------------------

#[test]
fn a_latitude_and_longitude_finds_its_cell() {
    let (planet, _) = a_planet(0x99);
    let north = planet.cell_at(90.0, 0.0);
    assert!(planet.grid().position(north).z > 0.9);
    let equator = planet.cell_at(0.0, 0.0);
    assert!(planet.grid().position(equator).z.abs() < 0.15);
    // And the same place is the same cell however the longitude is written.
    assert_eq!(planet.cell_at(12.0, 190.0), planet.cell_at(12.0, -170.0));
}
