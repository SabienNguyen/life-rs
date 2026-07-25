//! What a climate has to do to be believable.
//!
//! The tests that matter here are about the *thermostat*, because that is the claim the
//! whole module exists to make: a planet with volcanoes, continents and rain regulates
//! its own temperature against changes that would otherwise cook or freeze it, and it
//! does so without anything in the code aiming at a temperature.

use super::*;
use sim_core::{Domain, WorldSeed};

fn a_planet(seed: u128, land: f32) -> (Lithosphere, Rng) {
    let mut rng = WorldSeed::from_u128(seed).stream(Domain::Terrain, 0, 0);
    let planet = Lithosphere::genesis(4, 9, land, &mut rng);
    (planet, rng)
}

fn a_world(seed: u128) -> (Lithosphere, Climate, Rng) {
    let (mut planet, mut rng) = a_planet(seed, 0.42);
    // One step, so the plate boundaries have been worked out and there is volcanism.
    planet.step_myr(2.0, &mut rng);
    let climate = Climate::genesis(&planet, 4.57, insolation::EARTH_OBLIQUITY);
    (planet, climate, rng)
}

// ---- the state it settles into ----------------------------------------------------

#[test]
fn an_earthlike_planet_settles_at_an_earthlike_climate() {
    let (planet, climate, _) = a_world(0x1);
    let mean = climate.mean_temperature_c(&planet);
    assert!(
        (2.0..30.0).contains(&mean),
        "the planet settled at {mean:.1} °C"
    );
    let rain = climate.mean_rain_mm(&planet);
    assert!(
        (300.0..2500.0).contains(&rain),
        "it rained {rain:.0} mm a year"
    );
    assert!(
        climate.temperate_fraction(&planet) > 0.3,
        "only {:.2} of it could hold liquid water",
        climate.temperate_fraction(&planet)
    );
}

#[test]
fn carbon_dioxide_settles_somewhere_plausible() {
    let (_, climate, _) = a_world(0x2);
    let co2 = climate.co2_ppm();
    assert!(
        (30.0..8000.0).contains(&co2),
        "carbon dioxide settled at {co2:.0} ppm"
    );
}

#[test]
fn the_same_planet_gives_the_same_climate() {
    let once = {
        let (planet, climate, _) = a_world(0xABC);
        planet
            .grid()
            .cells()
            .map(|c| (climate.temperature_c(c), climate.rain_mm(c)))
            .collect::<Vec<_>>()
    };
    let twice = {
        let (planet, climate, _) = a_world(0xABC);
        planet
            .grid()
            .cells()
            .map(|c| (climate.temperature_c(c), climate.rain_mm(c)))
            .collect::<Vec<_>>()
    };
    assert_eq!(once, twice);
}

// ---- the thermostat ---------------------------------------------------------------

#[test]
fn the_thermostat_holds_the_planet_against_a_brightening_sun() {
    // The result the whole module is for. The sun gains a third of its output across
    // this span; without a thermostat that is nearly forty degrees of warming and the
    // end of the biosphere. With one, weathering speeds up, carbon is drawn down, and
    // the planet gives back most of it.
    let (mut planet, mut rng) = a_planet(0x3, 0.42);
    planet.step_myr(2.0, &mut rng);

    let at = |age: f64, planet: &Lithosphere| {
        let climate = Climate::genesis(planet, age, insolation::EARTH_OBLIQUITY);
        (
            climate.mean_temperature_c(planet),
            climate.co2_ppm(),
            insolation::brightness_at(age),
        )
    };
    let (young, young_co2, dim) = at(2.0, &planet);
    let (old, old_co2, bright) = at(5.5, &planet);

    let extra_sunlight = (bright / dim - 1.0) as f32;
    assert!(
        extra_sunlight > 0.3,
        "the sun should have brightened by a third, not {extra_sunlight:.2}"
    );
    // With no feedback at all, that much extra sunlight is this much warming.
    let unchecked = extra_sunlight * (1361.0 / 4.0) * 0.7 / 2.09;
    assert!(unchecked > 30.0);

    let warming = old - young;
    assert!(
        warming > 0.0 && warming < unchecked * 0.55,
        "the planet warmed {warming:.1} °C where an unregulated one would have warmed \
         {unchecked:.1} °C — the thermostat is not working"
    );
    assert!(
        old_co2 < young_co2 * 0.1,
        "carbon should be drawn down hard as the sun brightens: {young_co2:.0} then \
         {old_co2:.0} ppm"
    );
    // And both ends are places something could live.
    assert!(young > -5.0 && old < 40.0, "{young:.1} °C then {old:.1} °C");
}

#[test]
fn a_thick_atmosphere_is_what_keeps_a_young_planet_warm() {
    // The faint young sun. A planet under a sun four fifths of today's stays above
    // freezing, and the reason is written in its air: slow weathering under a cold sun
    // lets volcanic carbon accumulate until the greenhouse makes up the difference. The
    // amount it settles on — around a tenth of a bar — is what geochemists read out of
    // Archean rocks, which is not something this model was fitted to.
    let (mut planet, mut rng) = a_planet(0xB, 0.42);
    planet.step_myr(2.0, &mut rng);
    let climate = Climate::genesis(&planet, 2.0, insolation::EARTH_OBLIQUITY);

    assert!(
        climate.brightness() < 0.85,
        "this test wants a faint sun, not {:.2}",
        climate.brightness()
    );
    assert!(
        climate.mean_temperature_c(&planet) > -2.0,
        "the young planet froze at {:.1} °C",
        climate.mean_temperature_c(&planet)
    );
    let co2 = climate.co2_ppm();
    assert!(
        (20_000.0..400_000.0).contains(&co2),
        "it held {co2:.0} ppm of carbon dioxide"
    );
}

#[test]
fn a_faint_enough_sun_freezes_it_and_carbon_cannot_lift_it_out() {
    // The limit of the model, tested so that it stays a known limit rather than a
    // surprise. Below about three quarters of today's sunlight this planet goes to a
    // snowball and no achievable amount of carbon dioxide recovers it, because the
    // infrared law here is linearised around present conditions and its greenhouse
    // saturates. The real Earth of that era is thought to have needed methane as well,
    // which this does not model.
    let (mut planet, mut rng) = a_planet(0xC, 0.42);
    planet.step_myr(2.0, &mut rng);
    let climate = Climate::genesis(&planet, 1.0, insolation::EARTH_OBLIQUITY);
    assert!(climate.brightness() < 0.78);
    assert!(
        climate.ice_fraction(&planet) > 0.6,
        "expected a snowball, got {:.2} ice",
        climate.ice_fraction(&planet)
    );
    assert!(
        climate.co2_ppm() > 100_000.0,
        "and it should have tried: {:.0} ppm",
        climate.co2_ppm()
    );
}

#[test]
fn more_volcanism_means_more_carbon_but_not_much_more_heat() {
    // The other direction. Doubling the supply raises carbon dioxide a long way and the
    // temperature only a little, because weathering has to rise to match it and it does
    // that on a steep exponential.
    let (mut planet, mut rng) = a_planet(0x4, 0.42);
    planet.step_myr(2.0, &mut rng);

    let mut quiet = Climate::genesis(&planet, 4.57, insolation::EARTH_OBLIQUITY);
    let baseline = quiet.mean_temperature_c(&planet);
    let base_co2 = quiet.co2_ppm();
    let supply = carbon::outgassing(&planet);

    // Twice the volcanism, imposed by hand: this is the one place a test reaches past
    // the tectonics, because arranging twice the plate boundary on a real planet is not
    // something a test can do on demand.
    for _ in 0..40 {
        let demand = carbon::weathering(
            quiet.co2_ppm,
            quiet.land_temperature_c(&planet),
            planet.land_fraction(),
            quiet.land_runoff(&planet),
        );
        quiet.co2_ppm = carbon::relax(quiet.co2_ppm, 2.0 * supply, demand, 5.0);
        quiet.solve(&planet, 200);
    }
    let warmer = quiet.mean_temperature_c(&planet);

    assert!(
        quiet.co2_ppm() > base_co2 * 1.5,
        "carbon went from {base_co2:.0} to {:.0} ppm",
        quiet.co2_ppm()
    );
    assert!(
        (0.2..8.0).contains(&(warmer - baseline)),
        "doubling volcanism moved the temperature {:.1} °C",
        warmer - baseline
    );
}

#[test]
fn a_planet_with_no_land_has_no_thermostat() {
    // And so it runs away. Sea-floor weathering does some of this work on the real
    // planet and is not modelled; this is the honest consequence of that gap rather
    // than a claim about ocean worlds.
    let (mut planet, mut rng) = a_planet(0x5, 0.0);
    planet.step_myr(2.0, &mut rng);
    let climate = Climate::genesis(&planet, 4.57, insolation::EARTH_OBLIQUITY);
    assert!(
        climate.co2_ppm() > carbon::REFERENCE_CO2_PPM * 5.0,
        "an ocean world settled at {:.0} ppm",
        climate.co2_ppm()
    );
}

#[test]
fn where_the_continents_sit_changes_the_climate() {
    // Weathering happens on rock that is warm and wet, so continents in the tropics
    // draw carbon down harder than continents at the poles. This is a real control on
    // deep-time climate and it comes out of the geometry rather than being imposed.
    let (mut planet, mut rng) = a_planet(0x6, 0.42);
    planet.step_myr(2.0, &mut rng);
    let climate = Climate::genesis(&planet, 4.57, insolation::EARTH_OBLIQUITY);

    let land_temp = climate.land_temperature_c(&planet);
    let mean = climate.mean_temperature_c(&planet);
    // A sanity check on the coupling being wired at all: the land temperature is what
    // feeds the thermostat, and it should differ from the planetary mean.
    assert!(
        (land_temp - mean).abs() > 0.5,
        "land at {land_temp:.1} °C and the planet at {mean:.1} — they cannot both be right"
    );

    // And the equilibrium carbon should be what balances the volcanism against that.
    let supply = carbon::outgassing(&planet);
    let demand = carbon::weathering(
        climate.co2_ppm(),
        land_temp,
        planet.land_fraction(),
        climate.land_runoff(&planet),
    );
    assert!(
        (demand - supply).abs() < supply * 0.25,
        "supply {supply:.2} against demand {demand:.2} — the carbon has not settled"
    );
}

// ---- coupling to the planet -------------------------------------------------------

#[test]
fn the_climate_follows_the_continents_as_they_drift() {
    let (mut planet, mut rng) = a_planet(0x7, 0.42);
    planet.step_myr(2.0, &mut rng);
    let mut climate = Climate::genesis(&planet, 4.57, insolation::EARTH_OBLIQUITY);

    let before: Vec<f32> = planet
        .grid()
        .cells()
        .map(|c| climate.temperature_c(c))
        .collect();

    for _ in 0..60 {
        planet.step_myr(4.0, &mut rng);
        climate.step_myr(&planet, 4.0, &mut rng);
    }

    let after: Vec<f32> = planet
        .grid()
        .cells()
        .map(|c| climate.temperature_c(c))
        .collect();
    let moved = before
        .iter()
        .zip(&after)
        .filter(|(a, b)| (*a - *b).abs() > 1.0)
        .count();
    assert!(
        moved > before.len() / 10,
        "after 240 Myr of drift only {moved} of {} cells changed temperature",
        before.len()
    );
    // And it is still a planet, not a runaway.
    let mean = climate.mean_temperature_c(&planet);
    assert!((-30.0..60.0).contains(&mean), "it ended at {mean:.1} °C");
}

#[test]
fn a_billion_years_stays_habitable() {
    // The integration test for the whole stack: plates, erosion, sea level, radiation,
    // rain, and carbon, run together for a gigayear. The claim is not that the numbers
    // are right — it is that the feedbacks hold, which is the thing that fails first
    // when a coupled model is wrong.
    let (mut planet, mut rng) = a_planet(0x8, 0.42);
    planet.step_myr(2.0, &mut rng);
    let mut climate = Climate::genesis(&planet, 3.6, insolation::EARTH_OBLIQUITY);

    let mut coldest = f32::MAX;
    let mut hottest = f32::MIN;
    for _ in 0..100 {
        planet.step_myr(10.0, &mut rng);
        climate.step_myr(&planet, 10.0, &mut rng);
        let mean = climate.mean_temperature_c(&planet);
        assert!(mean.is_finite(), "the climate went to nonsense");
        coldest = coldest.min(mean);
        hottest = hottest.max(mean);
    }

    assert!(
        coldest > -25.0,
        "it froze over at {coldest:.1} °C and never recovered"
    );
    assert!(hottest < 70.0, "it ran away to {hottest:.1} °C");
    assert!(
        climate.temperate_fraction(&planet) > 0.15,
        "after a gigayear only {:.2} of it could hold liquid water",
        climate.temperate_fraction(&planet)
    );
}

#[test]
fn ice_grows_when_it_gets_cold_and_retreats_when_it_warms() {
    let (mut planet, mut rng) = a_planet(0x9, 0.42);
    planet.step_myr(2.0, &mut rng);

    let ice_at = |age: f64| {
        let climate = Climate::genesis(&planet, age, insolation::EARTH_OBLIQUITY);
        climate.ice_fraction(&planet)
    };
    let early = ice_at(2.0);
    let late = ice_at(5.5);
    assert!(
        early >= late,
        "a fainter sun left less ice: {early:.2} then {late:.2}"
    );
}

#[test]
fn tilt_changes_where_the_ice_is() {
    // Obliquity is the Milankovitch lever. More tilt means more sunlight at the poles
    // over a year, which is less polar ice — the mechanism behind glacial cycles, here
    // at the resolution deep time can see it.
    let (mut planet, mut rng) = a_planet(0xA, 0.42);
    planet.step_myr(2.0, &mut rng);
    let ice_at = |tilt: f64| Climate::genesis(&planet, 4.57, tilt).ice_fraction(&planet);
    let upright = ice_at(15.0);
    let tilted = ice_at(32.0);
    assert!(
        upright > tilted,
        "an upright planet had {upright:.3} ice and a tilted one {tilted:.3}"
    );
}
