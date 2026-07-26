//! What the join has to get right.
//!
//! Two kinds of test here. The first kind is about the projection itself and runs on
//! hand-built extremes — a frozen cell, a desert cell, a mountain — because those are
//! where a multiplicative model either works or quietly does not. The second kind builds
//! a whole planet and asks whether the settlements it produces look like somewhere people
//! live: on land, spread out, off the ice, and near the sea more often than the map alone
//! would explain.

use super::*;
use sim_core::{Domain, WorldSeed};

const SEED: u128 = 0x50_1717;

/// A small planet with a climate and a biosphere, settled enough to read.
fn planet() -> (Lithosphere, Climate, Biosphere) {
    let seed = WorldSeed::from_u128(SEED);
    let mut rng = seed.stream(Domain::Terrain, 0, 0);
    let mut planet = Lithosphere::genesis(3, 8, 0.40, &mut rng);
    planet.step_myr(4.0, &mut rng);
    let climate = Climate::genesis(&planet, 4.5, climate::insolation::EARTH_OBLIQUITY);
    let life = Biosphere::read(&planet, &climate);
    (planet, climate, life)
}

fn rng() -> Rng {
    WorldSeed::from_u128(SEED).stream(Domain::Chance, 0, 0)
}

#[test]
fn nobody_lives_in_the_sea() {
    let (planet, climate, life) = planet();
    let habitability = Habitability::of(&planet, &climate, &life);
    for cell in planet.grid().cells() {
        if !planet.is_land(cell) {
            assert_eq!(
                habitability.score(cell),
                0.0,
                "cell {cell} is under water and scored above zero"
            );
        }
    }
}

#[test]
fn every_settlement_is_on_dry_land() {
    let (planet, climate, life) = planet();
    let sites = survey(&planet, &climate, &life, 5, 1, &mut rng());
    assert!(!sites.is_empty(), "a planet with continents has somewhere to live");
    for site in &sites {
        assert!(
            planet.is_land(site.terrain.cell),
            "{} was founded in the ocean",
            site.name
        );
        assert!(site.habitability > 0.0);
    }
}

#[test]
fn settlements_are_not_founded_on_top_of_each_other() {
    let (planet, climate, life) = planet();
    let sites = survey(&planet, &climate, &life, 5, 1, &mut rng());
    let grid = planet.grid();
    for (i, a) in sites.iter().enumerate() {
        for b in &sites[i + 1..] {
            assert!(
                !within(grid, a.terrain.cell, b.terrain.cell, 1),
                "{} and {} are neighbours, which makes them one town",
                a.name,
                b.name
            );
        }
    }
}

#[test]
fn the_quarters_of_one_town_are_in_one_country() {
    // The bug the map found and no statistic would have. The first version took the best
    // cells on the planet outright and put five neighbourhoods of one society on three
    // continents — 128° east, 75° west, 165° east — which is not a town, it is five
    // unrelated civilisations that happen to share a chronicle.
    let (planet, climate, life) = planet();
    let sites = survey(&planet, &climate, &life, 5, 1, &mut rng());
    assert!(sites.len() >= 3, "only {} sites", sites.len());

    let grid = planet.grid();
    let home = sites[0].terrain.cell;
    for site in &sites[1..] {
        let apart = grid.distance_km(home, site.terrain.cell, geo::EARTH_RADIUS_KM);
        assert!(
            apart < 5_000.0,
            "{} is {apart:.0} km from {} — that is not the same society",
            site.name,
            sites[0].name
        );
    }
}

#[test]
fn a_region_is_varied_rather_than_five_copies_of_its_best_cell() {
    // The other half of the same correction: bounding the search to one country is only
    // worth doing if the country has good and bad ground in it.
    let (planet, climate, life) = planet();
    let sites = survey(&planet, &climate, &life, 5, 1, &mut rng());
    let best = sites
        .iter()
        .map(|s| s.habitability)
        .fold(f32::MIN, f32::max);
    let worst = sites
        .iter()
        .map(|s| s.habitability)
        .fold(f32::MAX, f32::min);
    assert!(
        best > worst * 1.15,
        "every quarter scored the same ({worst:.3}..{best:.3}); the region has no shape"
    );
}

#[test]
fn a_frozen_cell_is_uninhabitable_however_good_the_rest_of_it_is() {
    // The property a product buys and a sum does not. Everything about this cell is
    // excellent except that it is under ice, and a weighted sum would happily put a
    // capital city on it.
    let (planet, climate, life) = planet();
    let habitability = Habitability::of(&planet, &climate, &life);
    let frozen: Vec<_> = planet
        .grid()
        .cells()
        .filter(|&c| planet.is_land(c) && climate.is_frozen(c))
        .collect();
    if frozen.is_empty() {
        return; // A warm world need not have any, and that is not a failure.
    }
    for cell in frozen {
        assert_eq!(habitability.harshness(cell), 1.0);
        assert_eq!(habitability.score(cell), 0.0);
    }
}

#[test]
fn the_coast_is_more_reachable_than_the_interior() {
    let (planet, climate, life) = planet();
    let habitability = Habitability::of(&planet, &climate, &life);
    let grid = planet.grid();

    let mut coastal = (0.0, 0);
    let mut inland = (0.0, 0);
    for cell in grid.cells() {
        if !planet.is_land(cell) {
            continue;
        }
        let on_sea = grid.neighbours(cell).iter().any(|&n| !planet.is_land(n));
        let bucket = if on_sea { &mut coastal } else { &mut inland };
        bucket.0 += habitability.reach(cell) as f64;
        bucket.1 += 1;
    }
    assert!(coastal.1 > 0 && inland.1 > 0, "the planet needs both");
    assert!(
        coastal.0 / coastal.1 as f64 > inland.0 / inland.1 as f64,
        "the sea is a road, so the coast should be the easier place to reach"
    );
}

#[test]
fn a_mountain_is_harsher_than_the_valley_under_it() {
    let (planet, climate, life) = planet();
    let habitability = Habitability::of(&planet, &climate, &life);
    let grid = planet.grid();
    let highest = grid
        .cells()
        .filter(|&c| planet.is_land(c))
        .max_by(|a, b| {
            planet
                .height_above_sea_m(*a)
                .total_cmp(&planet.height_above_sea_m(*b))
        })
        .unwrap();
    if planet.height_above_sea_m(highest) < TOO_HIGH_M {
        return;
    }
    let lowest = grid
        .cells()
        .filter(|&c| planet.is_land(c))
        .min_by(|a, b| {
            planet
                .height_above_sea_m(*a)
                .total_cmp(&planet.height_above_sea_m(*b))
        })
        .unwrap();
    assert!(habitability.harshness(highest) > habitability.harshness(lowest));
}

#[test]
fn carrying_capacity_follows_the_land_rather_than_being_authored() {
    let (planet, climate, life) = planet();
    let habitability = Habitability::of(&planet, &climate, &life);
    let centre = heartland(&planet, &habitability, &mut rng()).unwrap();
    let sites = sites(&planet, &habitability, centre, 4, 20, 1, &mut rng());
    assert!(sites.len() > 1);

    let best = sites
        .iter()
        .max_by(|a, b| a.terrain.fertility.total_cmp(&b.terrain.fertility))
        .unwrap();
    let worst = sites
        .iter()
        .min_by(|a, b| a.terrain.fertility.total_cmp(&b.terrain.fertility))
        .unwrap();
    assert!(
        best.terrain.carrying >= worst.terrain.carrying,
        "better land should hold more people, not fewer"
    );
    for site in &sites {
        assert!(site.terrain.carrying >= 1, "nowhere holds nobody");
    }
}

#[test]
fn the_best_sites_come_first() {
    let (planet, climate, life) = planet();
    let sites = survey(&planet, &climate, &life, 5, 1, &mut rng());
    // Not monotone — spacing displaces some good cells — but the first found should beat
    // the last, or the ranking is not doing anything.
    assert!(
        sites[0].habitability > sites[sites.len() - 1].habitability,
        "the first site chosen should be better than the last"
    );
}

#[test]
fn a_site_knows_what_grows_on_it() {
    let (planet, climate, life) = planet();
    let sites = survey(&planet, &climate, &life, 5, 1, &mut rng());
    for site in &sites {
        assert!(!site.terrain.biome.is_empty(), "{} has no biome", site.name);
        assert_eq!(site.terrain.biome, life.biome(site.terrain.cell).label());
        assert!(
            !life.biome(site.terrain.cell).is_marine(),
            "{} is in a marine biome",
            site.name
        );
    }
}

#[test]
fn richer_ground_permits_a_richer_place() {
    let mut poor = Terrain::middling(0);
    poor.fertility = 0.05;
    poor.reach = 0.1;
    let mut good = Terrain::middling(1);
    good.fertility = 0.9;
    good.reach = 0.8;
    assert!(poor.prosperity_ceiling() < good.prosperity_ceiling());
    // And the poor one is not zero: hard country is poor, not uninhabitable.
    assert!(poor.prosperity_ceiling() > 0.2);
}

#[test]
fn the_same_planet_and_seed_settle_the_same_way() {
    let (planet, climate, life) = planet();
    let first = survey(&planet, &climate, &life, 5, 1, &mut rng());
    let second = survey(&planet, &climate, &life, 5, 1, &mut rng());
    let names = |sites: &[Site]| -> Vec<(String, u32)> {
        sites
            .iter()
            .map(|s| (s.name.clone(), s.terrain.cell))
            .collect()
    };
    assert_eq!(names(&first), names(&second));
}

#[test]
fn different_worlds_settle_differently() {
    let (planet, climate, life) = planet();
    let mut a = WorldSeed::from_u128(1).stream(Domain::Chance, 0, 0);
    let mut b = WorldSeed::from_u128(2).stream(Domain::Chance, 0, 0);
    let first = survey(&planet, &climate, &life, 5, 1, &mut a);
    let second = survey(&planet, &climate, &life, 5, 1, &mut b);
    let names = |sites: &[Site]| -> Vec<String> { sites.iter().map(|s| s.name.clone()).collect() };
    assert_ne!(names(&first), names(&second));
}

#[test]
fn most_of_a_planet_is_somewhere_nobody_would_live() {
    let (planet, climate, life) = planet();
    let habitability = Habitability::of(&planet, &climate, &life);
    let share = habitability.habitable_fraction(&planet);
    // Land is about a third of the surface and most of that is too cold, too dry or too
    // high. A model that says half the planet is comfortable has lost the plot.
    assert!(
        (0.01..0.35).contains(&share),
        "{:.1}% of the surface came out habitable",
        share * 100.0
    );
}

#[test]
fn asking_for_more_sites_than_the_planet_has_is_not_an_error() {
    let (planet, climate, life) = planet();
    let sites = survey(&planet, &climate, &life, 100_000, 1, &mut rng());
    assert!(!sites.is_empty());
    assert!(sites.len() < 100_000, "the planet ran out, as it should");
}

#[test]
fn farmland_is_the_temperate_and_tropical_middle() {
    assert!(is_farmable(Biome::Grassland));
    assert!(is_farmable(Biome::TemperateForest));
    assert!(!is_farmable(Biome::Glacier));
    assert!(!is_farmable(Biome::Tundra));
    assert!(!is_farmable(Biome::Desert));
    assert!(!is_farmable(Biome::Pelagic));
}
