//! What an ecology has to do to be worth having.
//!
//! The claims here are the ones ecology textbooks open with — the trophic pyramid, the
//! latitudinal diversity gradient, ranges that track climate — and none of them is coded
//! for. They are what falls out of a productivity field, a tenth passed up each level,
//! and species that can only live where they can live.

use super::*;
use climate::{Climate, insolation};

fn a_world(seed: u128, species: usize) -> (Lithosphere, Climate, Biosphere, Ecology, Rng) {
    let world = WorldSeed::from_u128(seed);
    let mut rng = world.stream(Domain::Terrain, 0, 0);
    let mut planet = Lithosphere::genesis(4, 9, 0.42, &mut rng);
    for _ in 0..25 {
        planet.step_myr(4.0, &mut rng);
    }
    let climate = Climate::genesis(&planet, 4.57, insolation::EARTH_OBLIQUITY);
    let life = Biosphere::read(&planet, &climate);
    let mut ecology = Ecology::genesis(&planet, &life, &climate, species, world);
    // A few steps to let the populations find their level from the token seeding.
    for _ in 0..6 {
        ecology.step_myr(&planet, &life, &climate, 1.0, &mut rng);
    }
    (planet, climate, life, ecology, rng)
}

// ---- the pyramid --------------------------------------------------------------------

#[test]
fn there_is_more_herbivore_than_carnivore_by_about_the_right_factor() {
    // Thermodynamics, not biology. Each step up keeps about a tenth, so a planet with
    // ten times as much grazer as predator is a planet whose arithmetic is right.
    let (_, _, _, ecology, _) = a_world(0x1, 40);
    let grazers = ecology.biomass_at_mt(Trophic::Herbivore);
    let hunters = ecology.biomass_at_mt(Trophic::Carnivore);
    assert!(
        grazers > 0.0 && hunters > 0.0,
        "one level died out entirely"
    );
    let ratio = grazers / hunters;
    assert!(
        (3.0..200.0).contains(&ratio),
        "there was {ratio:.1} times as much herbivore as carnivore"
    );
}

#[test]
fn the_animals_weigh_far_less_than_a_years_growth() {
    // Standing animal biomass is a rounding error against what the plants make. If it
    // were not, the food chain would be taking more than exists.
    let (planet, _, life, ecology, _) = a_world(0x2, 40);
    let growth_mt = life.total_production_gt(&planet) * 1000.0;
    let animals_mt = ecology.total_biomass_mt();
    assert!(animals_mt > 0.0, "nothing was alive");
    assert!(
        animals_mt < growth_mt * 0.2,
        "the animals weighed {animals_mt:.0} Mt against {growth_mt:.0} Mt of annual growth"
    );
}

#[test]
fn productive_places_carry_more_animal_than_barren_ones() {
    let (planet, _, life, ecology, _) = a_world(0x3, 40);
    let grid = planet.grid();

    let total_at = |cell: CellId| {
        ecology
            .living()
            .map(|id| ecology.biomass_of(id, cell))
            .sum::<f32>()
    };
    let mut rich = (0.0f32, 0usize);
    let mut poor = (0.0f32, 0usize);
    for cell in grid.cells() {
        if life.biome(cell).is_marine() {
            continue;
        }
        let per_km2 = total_at(cell) / grid.area_km2(cell, geo::EARTH_RADIUS_KM) as f32;
        if life.production(cell) > 1200.0 {
            rich.0 += per_km2;
            rich.1 += 1;
        } else if life.production(cell) < 200.0 {
            poor.0 += per_km2;
            poor.1 += 1;
        }
    }
    assert!(rich.1 > 3 && poor.1 > 3, "{} rich, {} poor", rich.1, poor.1);
    assert!(
        rich.0 / rich.1 as f32 > poor.0 / poor.1 as f32 * 2.0,
        "rich land carried {:.2} t/km² and poor land {:.2}",
        rich.0 / rich.1 as f32,
        poor.0 / poor.1 as f32
    );
}

// ---- where things live ---------------------------------------------------------------

#[test]
fn there_are_more_kinds_of_animal_in_the_tropics() {
    // The latitudinal diversity gradient, which is the most conspicuous pattern in all of
    // biogeography. Nothing here reaches for it: the tropics are warm, wet and productive,
    // so more species can meet their requirements there and there is more to go round.
    let (planet, _, _, ecology, _) = a_world(0x4, 60);
    let grid = planet.grid();

    let mean_richness = |low: f64, high: f64| {
        let cells: Vec<CellId> = grid
            .cells()
            .filter(|c| {
                let lat = grid.position(*c).latitude().to_degrees().abs();
                (low..high).contains(&lat)
            })
            .collect();
        cells.iter().map(|c| ecology.richness_at(*c)).sum::<usize>() as f32 / cells.len() as f32
    };

    let tropics = mean_richness(0.0, 20.0);
    let poles = mean_richness(65.0, 91.0);
    assert!(
        tropics > poles * 1.3,
        "the tropics held {tropics:.1} species a cell and the poles {poles:.1}"
    );
}

#[test]
fn a_species_only_lives_where_it_can_live() {
    let (planet, climate, life, ecology, _) = a_world(0x5, 40);
    for id in ecology.living() {
        let species = ecology.get(id);

        for cell in planet.grid().cells() {
            if ecology.biomass_of(id, cell) <= ecology.presence_floor(cell) {
                continue;
            }
            let suits = species.suitability(
                climate.temperature_c(cell),
                climate.rain_mm(cell),
                life.biome(cell),
            );
            assert!(
                suits > 0.0,
                "{} is living somewhere it cannot: {} at {:.1} °C",
                species.name,
                life.biome(cell).label(),
                climate.temperature_c(cell)
            );
        }
    }
}

#[test]
fn a_species_range_spans_the_temperatures_it_can_stand_and_no_others() {
    // The direct claim about what a tolerance band means. Not "specialists have smaller
    // ranges", which is true of the real world for reasons — history, dispersal, area —
    // that this does not model, and which comes out noisy over a few dozen species.
    let (planet, climate, _, ecology, _) = a_world(0x6, 60);
    let grid = planet.grid();

    let mut checked = 0;
    for id in ecology.living() {
        let species = ecology.get(id);
        let occupied: Vec<f32> = grid
            .cells()
            .filter(|c| ecology.biomass_of(id, *c) > ecology.presence_floor(*c))
            .map(|c| climate.temperature_c(c))
            .collect();
        if occupied.len() < 4 {
            continue;
        }
        checked += 1;

        let coldest = occupied.iter().copied().fold(f32::MAX, f32::min);
        let warmest = occupied.iter().copied().fold(f32::MIN, f32::max);
        // The taper lets a species hold ground a few degrees past its band, and no more.
        assert!(
            coldest > species.coldest_c - 6.0 && warmest < species.warmest_c + 6.0,
            "{} tolerates {:.0}..{:.0} °C and is living across {:.0}..{:.0}",
            species.name,
            species.coldest_c,
            species.warmest_c,
            coldest,
            warmest
        );
        assert!(
            warmest - coldest <= species.tolerance_c() + 12.0,
            "{} spans {:.0} °C of ground on a {:.0} °C tolerance",
            species.name,
            warmest - coldest,
            species.tolerance_c()
        );
    }
    assert!(
        checked > 10,
        "only {checked} species had a range worth checking"
    );
}

#[test]
fn nothing_lives_on_the_ice() {
    let (planet, _, life, ecology, _) = a_world(0x7, 40);
    for cell in planet.grid().cells() {
        if life.biome(cell) == biome::Biome::Glacier {
            assert_eq!(ecology.richness_at(cell), 0, "something lives on a glacier");
        }
    }
}

// ---- over deep time --------------------------------------------------------------------

#[test]
fn rarity_carries_risk_and_some_species_do_not_make_it() {
    // Counted from genesis rather than from the settled state, because the losses come
    // where they should: in the first few megayears, as species that were seeded across
    // everywhere they could tolerate find out how little of that they can actually hold
    // against everything else claiming it.
    //
    // What is *not* here is turnover — nothing new arises, because nothing evolves yet.
    // So a run this long thins and never refills, and the test says so: losses happen,
    // and they are not a massacre.
    let (mut planet, mut climate, mut life, mut ecology, mut rng) = a_world(0x8, 60);
    let _ = &life;
    let after_settling = ecology.richness();
    assert!(ecology.lost > 0, "every species seeded survived contact");

    for _ in 0..40 {
        planet.step_myr(5.0, &mut rng);
        climate.step_myr(&planet, 5.0, &mut rng);
        life = Biosphere::read(&planet, &climate);
        ecology.step_myr(&planet, &life, &climate, 5.0, &mut rng);
    }

    let left = ecology.richness();
    assert!(left <= after_settling);
    assert!(
        left > 60 / 3,
        "only {left} of 60 species were left; that is a mass extinction every step"
    );
}

#[test]
fn a_species_fares_worse_when_the_world_it_needs_stops_existing() {
    // Two runs from the same start, differing in one thing: whether the sun brightens.
    // A specialist built for the cold end of the planet is put into both.
    //
    // Framed as a comparison rather than as "it survives here and dies there", because
    // the model has a second thing to say and it is also true: a narrow specialist is
    // fragile *before* anything happens to it. Rarity carries risk, and a species holding
    // one corner of a planet against two dozen competitors may not last either way. What
    // the warming has to do is make it reliably worse.
    let world = WorldSeed::from_u128(0xE);
    let mut setup = world.stream(Domain::Terrain, 0, 0);
    let mut planet = Lithosphere::genesis(4, 9, 0.42, &mut setup);
    for _ in 0..25 {
        planet.step_myr(4.0, &mut setup);
    }
    let cold = Climate::genesis(&planet, 3.0, insolation::EARTH_OBLIQUITY);
    let cold_life = Biosphere::read(&planet, &cold);
    let warm = Climate::genesis(&planet, 6.5, insolation::EARTH_OBLIQUITY);
    let warm_life = Biosphere::read(&planet, &warm);

    // Built for the coldest tenth of the *living* land — the ice itself is no use to
    // anything, so a band drawn over it would leave the species nowhere from the start.
    let mut temperatures: Vec<f32> = planet
        .grid()
        .cells()
        .filter(|c| planet.is_land(*c) && cold_life.biome(*c) != biome::Biome::Glacier)
        .map(|c| cold.temperature_c(c))
        .collect();
    temperatures.sort_by(f32::total_cmp);
    assert!(temperatures.len() > 40, "not enough living land to test on");
    let specialist = Species {
        name: "cold specialist".into(),
        trophic: Trophic::Herbivore,
        marine: false,
        mass_kg: 40.0,
        coldest_c: temperatures[0] - 5.0,
        warmest_c: temperatures[temperatures.len() / 10],
        driest_mm: 0.0,
        dispersal: 0.6,
        arose_myr: 0.0,
    };

    // The scenario has to be taking something away, or the test says nothing.
    let habitable = |climate: &Climate, life: &Biosphere| {
        planet
            .grid()
            .cells()
            .filter(|c| {
                specialist.suitability(
                    climate.temperature_c(*c),
                    climate.rain_mm(*c),
                    life.biome(*c),
                ) > 0.0
            })
            .count()
    };
    let (before, after) = (habitable(&cold, &cold_life), habitable(&warm, &warm_life));
    assert!(
        after * 2 < before,
        "the scenario took nothing away: {before} habitable cells became {after}"
    );

    let run = |climate: &Climate, life: &Biosphere| {
        let mut rng = world.stream(Domain::Ecology, 7, 0);
        let mut ecology = Ecology::genesis(&planet, &cold_life, &cold, 24, world);
        let id = ecology.introduce(&planet, &cold_life, specialist.clone());
        for _ in 0..8 {
            ecology.step_myr(&planet, &cold_life, &cold, 1.0, &mut rng);
        }
        for _ in 0..25 {
            ecology.step_myr(&planet, life, climate, 1.0, &mut rng);
        }
        if ecology.is_extinct(id) {
            0
        } else {
            ecology.range_of(id)
        }
    };

    let unchanged = run(&cold, &cold_life);
    let warmed = run(&warm, &warm_life);
    assert!(
        warmed * 2 < unchanged.max(1),
        "on an unchanged planet it held {unchanged} cells and on a warmed one {warmed}"
    );
}

#[test]
fn populations_do_not_blow_up_or_oscillate_away() {
    // The classic failure of a coupled predator-prey model: one level overshoots, the
    // other crashes, and the pair rings itself to pieces. Watched over two hundred
    // megayears, the totals should stay within an order of magnitude of themselves.
    let (mut planet, mut climate, mut life, mut ecology, mut rng) = a_world(0x9, 40);
    let _ = &life;

    let mut lowest = f32::MAX;
    let mut highest = 0.0f32;
    for _ in 0..40 {
        planet.step_myr(5.0, &mut rng);
        climate.step_myr(&planet, 5.0, &mut rng);
        life = Biosphere::read(&planet, &climate);
        ecology.step_myr(&planet, &life, &climate, 5.0, &mut rng);

        let total = ecology.total_biomass_mt();
        assert!(total.is_finite(), "biomass went to nonsense");
        if ecology.richness() > 0 {
            lowest = lowest.min(total);
            highest = highest.max(total);
        }
    }
    assert!(ecology.richness() > 0, "everything died");
    assert!(
        highest < lowest * 60.0,
        "total biomass ranged from {lowest:.0} to {highest:.0} Mt"
    );
}

#[test]
fn ranges_follow_the_climate_when_it_moves() {
    // The thing that makes this worth simulating rather than tabulating: a species does
    // not stay where it was put. Its range is wherever its requirements are met, and that
    // moves with the continents and with the sun.
    let (mut planet, mut climate, mut life, mut ecology, mut rng) = a_world(0xA, 40);
    let _ = &life;
    let before: Vec<usize> = ecology.living().map(|id| ecology.range_of(id)).collect();

    for _ in 0..30 {
        planet.step_myr(6.0, &mut rng);
        climate.step_myr(&planet, 6.0, &mut rng);
        life = Biosphere::read(&planet, &climate);
        ecology.step_myr(&planet, &life, &climate, 6.0, &mut rng);
    }
    let after: Vec<usize> = ecology.living().map(|id| ecology.range_of(id)).collect();

    let moved = before
        .iter()
        .zip(&after)
        .filter(|(a, b)| (**a as i64 - **b as i64).abs() > 4)
        .count();
    assert!(
        moved > after.len() / 4,
        "only {moved} of {} surviving ranges changed size in 180 Myr",
        after.len()
    );
}

#[test]
fn the_same_world_grows_the_same_ecology() {
    let read = || {
        let (_, _, _, ecology, _) = a_world(0xABC, 30);
        (
            ecology.richness(),
            ecology
                .living()
                .map(|id| (ecology.get(id).name.clone(), ecology.range_of(id)))
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(read(), read());
}

#[test]
fn species_have_names_that_say_something_about_them() {
    let (_, _, _, ecology, _) = a_world(0xB, 30);
    for id in ecology.living() {
        let species = ecology.get(id);
        assert!(!species.name.is_empty());
        if species.mass_kg > 300.0 {
            assert!(
                species.name.contains("great") || species.name.contains("large"),
                "a {:.0} kg animal is called {}",
                species.mass_kg,
                species.name
            );
        }
    }
}
