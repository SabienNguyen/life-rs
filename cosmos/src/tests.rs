//! What a star has to get right.
//!
//! Almost all of these check the model against measured stars, because a stellar model
//! with three constants in it has no business being wrong about the sun. The rest check
//! the consequence that matters to everything above this crate: a star's mass decides how
//! long anything living near it has, and the answer is not linear in anything.

use super::*;
use sim_core::{Domain, WorldSeed};

fn rng() -> Rng {
    WorldSeed::from_u128(0x5741).stream(Domain::World, 0, 0)
}

#[test]
fn the_sun_comes_out_as_the_sun() {
    let sun = Star::SUN;
    assert!(
        (0.98..1.02).contains(&sun.luminosity_solar()),
        "the sun read {:.3} solar luminosities",
        sun.luminosity_solar()
    );
    assert!(
        (5_600.0..5_950.0).contains(&sun.surface_k()),
        "the sun read {:.0} K",
        sun.surface_k()
    );
    assert_eq!(sun.colour(), "yellow");
    assert!(
        (1330.0..1400.0).contains(&sun.flux_at_au(1.0)),
        "the solar constant read {:.0} W/m²",
        sun.flux_at_au(1.0)
    );
    assert!((0.98..1.02).contains(&sun.earthlike_au()));
}

#[test]
fn the_faint_young_sun_is_faint() {
    // A quarter to a third down at four and a half gigayears ago, which is the whole
    // reason the carbon thermostat has to exist.
    let young = Star {
        mass_solar: 1.0,
        age_gyr: 0.1,
    };
    let ratio = young.luminosity_solar() / Star::SUN.luminosity_solar();
    assert!(
        (0.70..0.80).contains(&ratio),
        "the young sun was {:.2} of today's",
        ratio
    );
}

#[test]
fn heavy_stars_burn_out_and_light_ones_do_not() {
    // The single most important fact about stars for anything hoping to live near one.
    let lifetimes: Vec<f64> = [0.2, 0.5, 1.0, 1.5, 2.0]
        .iter()
        .map(|&m| main_sequence_gyr(m))
        .collect();
    for pair in lifetimes.windows(2) {
        assert!(pair[0] > pair[1], "a heavier star lasted longer: {lifetimes:?}");
    }
    assert!(
        (8.0..12.0).contains(&main_sequence_gyr(1.0)),
        "the sun lasts {:.1} Gyr",
        main_sequence_gyr(1.0)
    );
    // A star half again the sun's mass gets a few gigayears, not ten.
    assert!(main_sequence_gyr(1.5) < 3.0);
    // And a red dwarf outlives the universe several times over.
    assert!(main_sequence_gyr(0.2) > 100.0);
}

#[test]
fn a_red_dwarf_is_red_and_a_heavy_star_is_white() {
    let dwarf = Star {
        mass_solar: 0.15,
        age_gyr: 1.0,
    };
    let heavy = Star {
        mass_solar: 2.0,
        age_gyr: 0.3,
    };
    assert_eq!(dwarf.colour(), "red");
    assert!(dwarf.surface_k() < Star::SUN.surface_k());
    assert!(heavy.surface_k() > Star::SUN.surface_k());
    assert!(matches!(heavy.colour(), "white" | "yellow-white"));
}

#[test]
fn light_falls_off_as_the_square_of_the_distance() {
    let sun = Star::SUN;
    let near = sun.flux_at_au(1.0);
    let far = sun.flux_at_au(2.0);
    assert!((near / far - 4.0).abs() < 0.01, "{near} against {far}");
}

#[test]
fn keplers_third_law_holds() {
    let sun = Star::SUN;
    let earth = Orbit {
        semi_major_au: 1.0,
        mass_earth: 1.0,
    };
    assert!((earth.year_years(&sun) - 1.0).abs() < 0.01);
    // Four astronomical units is eight years, which is a cube root of a square and the
    // only thing worth testing about it.
    let far = Orbit {
        semi_major_au: 4.0,
        mass_earth: 1.0,
    };
    assert!((far.year_years(&sun) - 8.0).abs() < 0.05);
}

#[test]
fn the_sky_is_mostly_small_stars() {
    // The initial mass function is steep, and drawing uniformly in mass would make a
    // galaxy of blue giants — which is wrong, and worse, is a galaxy where nothing has
    // time to evolve.
    let mut rng = rng();
    let masses: Vec<f64> = (0..2000).map(|_| Star::drawn(&mut rng).mass_solar).collect();
    let below_sun = masses.iter().filter(|&&m| m < 1.0).count();
    assert!(
        below_sun as f64 / masses.len() as f64 > 0.7,
        "only {:.0}% of stars came out lighter than the sun",
        100.0 * below_sun as f64 / masses.len() as f64
    );
    // And the range is respected at both ends.
    for mass in &masses {
        assert!(
            (LIGHTEST_STAR..=HEAVIEST_STAR).contains(mass),
            "drew a star of {mass} solar masses"
        );
    }
}

#[test]
fn a_star_is_never_drawn_past_its_own_death() {
    let mut rng = rng();
    for _ in 0..500 {
        let star = Star::drawn(&mut rng);
        assert!(
            star.age_gyr < star.main_sequence_gyr(),
            "drew a {:.2}-solar-mass star aged {:.2} Gyr with a {:.2} Gyr life",
            star.mass_solar,
            star.age_gyr,
            star.main_sequence_gyr()
        );
        assert!(star.remaining_gyr() > 0.0);
    }
}

#[test]
fn a_system_puts_the_rock_inside_and_the_giants_outside() {
    let mut rng = rng();
    let mut inner_masses = Vec::new();
    let mut outer_masses = Vec::new();
    for _ in 0..60 {
        let system = System::drawn(&mut rng);
        let snow = 2.7 * system.star.earthlike_au();
        for world in &system.worlds {
            if world.semi_major_au < snow {
                inner_masses.push(world.mass_earth);
            } else {
                outer_masses.push(world.mass_earth);
            }
        }
    }
    assert!(!inner_masses.is_empty() && !outer_masses.is_empty());
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    assert!(
        mean(&outer_masses) > mean(&inner_masses) * 5.0,
        "inner worlds averaged {:.1} Earth masses and outer {:.1}",
        mean(&inner_masses),
        mean(&outer_masses)
    );
}

#[test]
fn orbits_are_ordered_and_never_on_top_of_each_other() {
    let mut rng = rng();
    for _ in 0..100 {
        let system = System::drawn(&mut rng);
        for pair in system.worlds.windows(2) {
            assert!(
                pair[1].semi_major_au > pair[0].semi_major_au * 1.3,
                "two worlds at {:.3} and {:.3} AU",
                pair[0].semi_major_au,
                pair[1].semi_major_au
            );
        }
    }
}

#[test]
fn most_systems_have_nowhere_worth_living() {
    // The point of the whole crate. A universe where every star has an Earth is not a
    // universe, it is a backdrop.
    let mut rng = rng();
    let mut lucky = 0;
    const TRIES: usize = 400;
    for _ in 0..TRIES {
        if System::drawn(&mut rng).best_world().is_some() {
            lucky += 1;
        }
    }
    // The measured quantity this stands against is `eta-Earth`, the share of stars with a
    // rocky planet in the habitable zone. Estimates run from about a tenth for sun-like
    // stars to about a half for red dwarfs, so a model landing anywhere in that band is
    // telling the truth and one landing at four in five is not.
    let share = lucky as f64 / TRIES as f64;
    assert!(
        (0.10..0.60).contains(&share),
        "{:.0}% of systems had a world worth living on",
        share * 100.0
    );
}

#[test]
fn the_world_a_system_picks_is_one_a_person_could_stand_on() {
    let mut rng = rng();
    let mut found = 0;
    for _ in 0..400 {
        let system = System::drawn(&mut rng);
        let Some(index) = system.best_world() else {
            continue;
        };
        found += 1;
        let world = system.worlds[index];
        assert!(world.is_rocky(), "picked a gas giant");
        assert!(
            habitability::zone(&system.star).holds(world.semi_major_au),
            "picked a world outside the habitable zone"
        );
        assert!(habitability::promise(&system.star, &world) > 0.0);
    }
    assert!(found > 5, "only {found} systems had a world at all");
}

#[test]
fn the_same_seed_draws_the_same_sky() {
    let mut a = rng();
    let mut b = rng();
    for _ in 0..50 {
        let first = System::drawn(&mut a);
        let second = System::drawn(&mut b);
        assert_eq!(first.star, second.star);
        assert_eq!(first.worlds, second.worlds);
    }
}

#[test]
fn a_bigger_world_pulls_harder() {
    let small = Orbit {
        semi_major_au: 1.0,
        mass_earth: 0.1,
    };
    let earth = Orbit {
        semi_major_au: 1.0,
        mass_earth: 1.0,
    };
    let large = Orbit {
        semi_major_au: 1.0,
        mass_earth: 5.0,
    };
    assert!(small.gravity() < earth.gravity());
    assert!(earth.gravity() < large.gravity());
    assert!((earth.gravity() - 1.0).abs() < 1e-9);
}
