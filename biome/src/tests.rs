//! What a biosphere on a real planet has to look like.
//!
//! The unit tests next door check that the classification recognises Manaus and Yakutsk.
//! These check the thing that actually matters and that no lookup table can fake: that
//! the pattern arranges itself sensibly over a whole sphere, and that it *moves* when the
//! planet underneath it does.

use super::*;
use climate::insolation;
use sim_core::{Domain, Rng, WorldSeed};

fn a_world(seed: u128) -> (Lithosphere, Climate, Rng) {
    let mut rng = WorldSeed::from_u128(seed).stream(Domain::Terrain, 0, 0);
    let mut planet = Lithosphere::genesis(4, 9, 0.42, &mut rng);
    for _ in 0..25 {
        planet.step_myr(4.0, &mut rng);
    }
    let climate = Climate::genesis(&planet, 4.57, insolation::EARTH_OBLIQUITY);
    (planet, climate, rng)
}

fn band<'a>(planet: &'a Lithosphere, low: f64, high: f64) -> impl Iterator<Item = CellId> + 'a {
    planet.grid().cells().filter(move |c| {
        let lat = planet.grid().position(*c).latitude().to_degrees().abs();
        (low..high).contains(&lat)
    })
}

// ---- the pattern -------------------------------------------------------------------

#[test]
fn an_earthlike_planet_grows_an_earthlike_spread_of_biomes() {
    let (planet, climate, _) = a_world(0x1);
    let life = Biosphere::read(&planet, &climate);
    let shares = life.shares(&planet);

    // A planet that is one biome everywhere is a planet whose classification is broken.
    let kinds = shares.iter().filter(|s| **s > 0.01).count();
    assert!(kinds >= 6, "only {kinds} biomes cover more than a percent");

    // And the ocean should dominate, because it does.
    let marine: f32 = (0..Biome::COUNT)
        .filter(|i| Biome::from_index(*i as u8).is_marine())
        .map(|i| shares[i])
        .sum();
    assert!(
        (0.6..0.95).contains(&marine),
        "the sea covered {marine:.2} of the planet"
    );
}

#[test]
fn the_warm_biomes_are_in_the_tropics_and_the_cold_ones_are_not() {
    // Stated as a pattern over the whole sphere rather than a rule about every cell,
    // because there are real exceptions: high ground in the tropics is cold ground, and
    // a planet whose thermostat has settled somewhere cool has its tree line further
    // down. What must hold is the arrangement.
    let (planet, climate, _) = a_world(0x2);
    let life = Biosphere::read(&planet, &climate);
    let grid = planet.grid();

    let mean_latitude = |wanted: &[Biome]| {
        let cells: Vec<CellId> = grid
            .cells()
            .filter(|c| wanted.contains(&life.biome(*c)))
            .collect();
        (!cells.is_empty()).then(|| {
            cells
                .iter()
                .map(|c| grid.position(*c).latitude().to_degrees().abs())
                .sum::<f64>()
                / cells.len() as f64
        })
    };

    let warm = mean_latitude(&[Biome::Rainforest, Biome::SeasonalForest, Biome::Savanna]);
    let cold = mean_latitude(&[Biome::Tundra, Biome::Glacier, Biome::Taiga]);
    if let (Some(warm), Some(cold)) = (warm, cold) {
        assert!(
            cold > warm + 20.0,
            "the cold biomes averaged {cold:.0}° and the warm ones {warm:.0}°"
        );
    }

    // And nowhere is a rainforest cold or a glacier warm, whatever the latitude.
    for cell in grid.cells() {
        let t = climate.temperature_c(cell);
        match life.biome(cell) {
            Biome::Rainforest | Biome::SeasonalForest => {
                assert!(t > 15.0, "a rainforest at {t:.1} °C")
            }
            Biome::Glacier => assert!(t < -10.0, "a glacier at {t:.1} °C"),
            Biome::Tundra => assert!(t < 10.0, "tundra at {t:.1} °C"),
            _ => {}
        }
    }
}

#[test]
fn there_is_a_belt_of_desert_in_the_subtropics() {
    // The same result the moisture model produces, now read as vegetation: the ring of
    // desert around thirty degrees is where the Hadley circulation puts it, and nothing
    // in this crate knows that.
    let (planet, climate, _) = a_world(0x3);
    let life = Biosphere::read(&planet, &climate);

    let arid_share = |low, high| {
        let cells: Vec<CellId> = band(&planet, low, high)
            .filter(|c| !life.biome(*c).is_marine())
            .collect();
        if cells.is_empty() {
            return 0.0;
        }
        cells.iter().filter(|c| life.biome(**c).is_arid()).count() as f32 / cells.len() as f32
    };

    let subtropics = arid_share(18.0, 38.0);
    let equator = arid_share(0.0, 12.0);
    assert!(
        subtropics > equator,
        "the subtropics were {subtropics:.2} arid and the equator {equator:.2}"
    );
}

#[test]
fn mountains_carry_their_own_biome() {
    // High ground is cold ground, so a range in the tropics wears tundra and ice above a
    // certain height. It falls out of the lapse rate rather than being placed.
    let (planet, climate, _) = a_world(0x4);
    let life = Biosphere::read(&planet, &climate);

    let high: Vec<CellId> = planet
        .grid()
        .cells()
        .filter(|c| planet.height_above_sea_m(*c) > 3000.0)
        .collect();
    assert!(high.len() > 4, "only {} cells of high ground", high.len());

    let cold = high
        .iter()
        .filter(|c| {
            matches!(
                life.biome(**c),
                Biome::Tundra | Biome::Glacier | Biome::Taiga
            )
        })
        .count();
    assert!(
        cold > 0,
        "not one of {} high cells was cold enough to change biome",
        high.len()
    );
}

// ---- how much of it there is --------------------------------------------------------

#[test]
fn the_planet_makes_an_earthlike_amount_of_matter() {
    // Earth fixes something like a hundred and twenty gigatonnes of dry matter a year,
    // split roughly evenly between land and sea. Nothing here is fitted to that number:
    // it comes out of the Miami model applied to whatever climate the planet has.
    let (planet, climate, _) = a_world(0x5);
    let life = Biosphere::read(&planet, &climate);

    let total = life.total_production_gt(&planet);
    assert!(
        (25.0..400.0).contains(&total),
        "the biosphere made {total:.0} Gt of dry matter a year"
    );
    let land = life.land_production_gt(&planet);
    assert!(
        land > 0.0 && land < total,
        "land made {land:.0} of a total {total:.0}"
    );
}

#[test]
fn production_follows_the_biome() {
    let (planet, climate, _) = a_world(0x6);
    let life = Biosphere::read(&planet, &climate);

    let mean_for = |wanted: Biome| {
        let cells: Vec<CellId> = planet
            .grid()
            .cells()
            .filter(|c| life.biome(*c) == wanted)
            .collect();
        (!cells.is_empty())
            .then(|| cells.iter().map(|c| life.production(*c)).sum::<f32>() / cells.len() as f32)
    };

    if let (Some(forest), Some(desert)) = (mean_for(Biome::Rainforest), mean_for(Biome::Desert)) {
        assert!(
            forest > desert * 5.0,
            "rainforest made {forest:.0} and desert {desert:.0}"
        );
    }
    for cell in planet.grid().cells() {
        if life.biome(cell) == Biome::Glacier {
            assert_eq!(life.production(cell), 0.0);
        }
        assert!(life.production(cell) >= 0.0);
        assert!(life.production(cell).is_finite());
    }
}

// ---- and that it moves ---------------------------------------------------------------

#[test]
fn the_biosphere_moves_when_the_planet_does() {
    // The whole point of deriving rather than storing. Nothing migrates here and nothing
    // is edited — the continents drift, the climate follows, and the vegetation is
    // simply read off somewhere else than it was.
    let (mut planet, mut climate, mut rng) = a_world(0x7);
    let before = Biosphere::read(&planet, &climate);
    let was: Vec<Biome> = planet.grid().cells().map(|c| before.biome(c)).collect();

    for _ in 0..50 {
        planet.step_myr(6.0, &mut rng);
        climate.step_myr(&planet, 6.0, &mut rng);
    }

    let after = Biosphere::read(&planet, &climate);
    let changed = planet
        .grid()
        .cells()
        .filter(|c| after.biome(*c) != was[*c as usize])
        .count();
    assert!(
        changed > planet.grid().len() / 20,
        "after 300 Myr of drift only {changed} of {} cells changed what grew on them",
        planet.grid().len()
    );
}

#[test]
fn a_warming_planet_loses_its_tundra() {
    // Same continents, same rain pattern, different sun. The cold biomes retreat towards
    // the pole and the warm ones follow them up, which is what every glacial cycle in the
    // record does at a smaller amplitude.
    let mut rng = WorldSeed::from_u128(0x8).stream(Domain::Terrain, 0, 0);
    let mut planet = Lithosphere::genesis(4, 9, 0.42, &mut rng);
    for _ in 0..25 {
        planet.step_myr(4.0, &mut rng);
    }

    let cold_share = |age: f64| {
        let climate = Climate::genesis(&planet, age, insolation::EARTH_OBLIQUITY);
        let life = Biosphere::read(&planet, &climate);
        let shares = life.shares(&planet);
        shares[Biome::Tundra as usize]
            + shares[Biome::Glacier as usize]
            + shares[Biome::Taiga as usize]
    };
    let cool = cold_share(3.0);
    let warm = cold_share(5.5);
    assert!(
        cool > warm,
        "a cooler planet had {cool:.3} of cold biome and a warmer one {warm:.3}"
    );
}

#[test]
fn the_same_world_reads_the_same_biosphere() {
    let read = || {
        let (planet, climate, _) = a_world(0xABC);
        let life = Biosphere::read(&planet, &climate);
        planet
            .grid()
            .cells()
            .map(|c| (life.biome(c), life.production(c)))
            .collect::<Vec<_>>()
    };
    assert_eq!(read(), read());
}

#[test]
fn a_snowball_has_almost_nothing_living_on_it() {
    let mut rng = WorldSeed::from_u128(0x9).stream(Domain::Terrain, 0, 0);
    let mut planet = Lithosphere::genesis(4, 9, 0.42, &mut rng);
    planet.step_myr(4.0, &mut rng);
    // A sun faint enough that the thermostat cannot save it.
    let climate = Climate::genesis(&planet, 1.0, insolation::EARTH_OBLIQUITY);
    let life = Biosphere::read(&planet, &climate);

    let total = life.total_production_gt(&planet);
    let warm = {
        let sunlit = Climate::genesis(&planet, 4.57, insolation::EARTH_OBLIQUITY);
        Biosphere::read(&planet, &sunlit).total_production_gt(&planet)
    };
    assert!(
        total < warm * 0.35,
        "a snowball made {total:.0} Gt against a temperate planet's {warm:.0}"
    );
}
