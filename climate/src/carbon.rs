//! The carbon–silicate thermostat: why a planet stays habitable for billions of years.
//!
//! Volcanoes put carbon dioxide into the air. Rain takes it out, by weathering silicate
//! rock into carbonate that ends up on the sea floor. The rate rock weathers rises
//! steeply with temperature and with how much rain falls on it — so a planet that warms
//! draws its own carbon down and cools, and a planet that cools stops weathering and
//! lets carbon accumulate until it warms again.
//!
//! That single negative feedback is the reason Earth has had liquid water for four
//! billion years while the sun brightened by a third, and it is the reason the faint
//! young sun did not leave the young Earth frozen. It is also the one place in this
//! whole simulation where tectonics, climate, and the surface meet in a loop:
//! **subduction sets the supply, and rain on continents sets the demand.** Move the
//! continents to the tropics and the planet cools itself; close the trenches and it
//! freezes.
//!
//! The numbers are the GEOCARB ones, which are themselves fitted to the geological
//! record rather than derived. The exponents are what matters and they are well
//! constrained: weathering goes as roughly the 0.3 power of carbon dioxide, the 0.65
//! power of runoff, and e-folds every fourteen degrees.

use geo::{Boundary, Lithosphere};

/// Carbon dioxide the modern rates are quoted at, in parts per million.
pub const REFERENCE_CO2_PPM: f32 = 300.0;
/// Global mean temperature the modern rates are quoted at, in °C.
pub const REFERENCE_TEMP_C: f32 = 14.0;
/// Share of the surface that is dry land in the reference state.
pub const REFERENCE_LAND: f32 = 0.29;
/// Share of cells sitting on a convergent or divergent boundary in the reference state.
///
/// A normalisation rather than a measurement: it converts "how much of this planet's
/// surface is a plate boundary" into "how many times the modern outgassing rate", and
/// its value is what a level-four Earth-like planet actually produces, so that such a
/// planet starts near its own equilibrium instead of spending a hundred megayears
/// walking there.
pub const REFERENCE_BOUNDARY: f32 = 0.16;

/// How steeply weathering responds to warming: it e-folds every this many degrees.
const TEMPERATURE_EFOLD: f32 = 13.7;
/// The exponent on carbon dioxide — direct effect on rock dissolution, apart from the
/// warming it also causes.
const CO2_EXPONENT: f32 = 0.3;
/// The exponent on runoff.
const RUNOFF_EXPONENT: f32 = 0.65;

/// How long the ocean-atmosphere carbon reservoir takes to respond, in megayears.
///
/// Short compared with a tectonic step, which is why the planet spends most of deep
/// time sitting close to whatever equilibrium the current arrangement of continents
/// implies, and only briefly away from it after something moves.
pub const RESPONSE_MYR: f32 = 0.4;

/// Carbon dioxide entering the air from volcanoes, relative to the modern rate.
///
/// Read off the plate boundaries. Subduction returns carbonate sediment to the mantle
/// and sends it back up through arc volcanoes; ridges outgas directly. Both scale with
/// how much boundary there is, which is a thing the tectonics already knows.
pub fn outgassing(planet: &Lithosphere) -> f32 {
    let grid = planet.grid();
    let mut boundary = 0.0;
    let mut total = 0.0;
    for cell in grid.cells() {
        let area = grid.solid_angle(cell);
        total += area;
        if matches!(
            planet.boundary(cell),
            Boundary::Convergent | Boundary::Divergent
        ) {
            boundary += area;
        }
    }
    ((boundary / total) as f32 / REFERENCE_BOUNDARY).clamp(0.05, 6.0)
}

/// Carbon dioxide removed by weathering, relative to the modern rate.
///
/// `land_temp_c` is the mean temperature of the land, not of the planet: weathering
/// happens on rock, and where the continents sit is exactly what the feedback is
/// sensitive to. `runoff` is relative to the modern mean, and `land` is the share of the
/// surface that is dry.
pub fn weathering(co2_ppm: f32, land_temp_c: f32, land: f32, runoff: f32) -> f32 {
    if land <= 0.0 {
        // A waterworld has no silicate weathering worth the name, and therefore no
        // thermostat. Sea-floor weathering does some of the work on the real planet; it
        // is not modelled here, and the honest consequence is that an ocean planet's
        // carbon runs away.
        return 0.0;
    }
    let by_co2 = (co2_ppm / REFERENCE_CO2_PPM).max(1e-6).powf(CO2_EXPONENT);
    let by_heat = ((land_temp_c - REFERENCE_TEMP_C) / TEMPERATURE_EFOLD).exp();
    let by_rain = runoff.max(0.0).powf(RUNOFF_EXPONENT);
    let by_area = land / REFERENCE_LAND;
    by_co2 * by_heat * by_rain * by_area
}

/// How much weathering changes for a given proportional change in carbon dioxide.
///
/// Not the 0.3 exponent above, because raising carbon dioxide also warms the planet and
/// warming is itself the strongest term in the weathering law. Direct effect 0.3, plus
/// about 2.5 K per e-folding of carbon divided by the 13.7 K e-folding of weathering,
/// gives a little under a half. It is used to invert the law — to ask what carbon
/// dioxide would balance a given supply — and getting it wrong does not change where the
/// equilibrium is, only how fast the planet walks to it.
const CO2_ELASTICITY: f32 = 0.48;

/// Move carbon dioxide towards the balance of supply and demand.
///
/// Both rates are relative to the modern ones. `demand` is what weathering is doing at
/// the *current* carbon dioxide; where it should end up is found by inverting the
/// weathering law, and then approached exponentially so that a span far longer than the
/// reservoir's response time lands on the answer instead of ringing past it.
pub fn relax(co2_ppm: f32, supply: f32, demand: f32, dt_myr: f32) -> f32 {
    if demand <= 0.0 {
        // Nothing is removing it. Accumulate, and let the greenhouse do what it will.
        return (co2_ppm + supply * REFERENCE_CO2_PPM * dt_myr / RESPONSE_MYR).min(500_000.0);
    }
    // Clamped, because a planet that has just frozen over reports almost no weathering
    // at all, and the unclamped inversion of that is a jump of several orders of
    // magnitude in a single step.
    let imbalance = (supply / demand).clamp(0.02, 50.0);
    let target = co2_ppm * imbalance.powf(1.0 / CO2_ELASTICITY);
    let blend = 1.0 - (-dt_myr / RESPONSE_MYR).exp();
    (co2_ppm + (target - co2_ppm) * blend).clamp(0.1, 500_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODERN: f32 = REFERENCE_CO2_PPM;

    #[test]
    fn the_reference_state_is_in_balance() {
        let w = weathering(MODERN, REFERENCE_TEMP_C, REFERENCE_LAND, 1.0);
        assert!((w - 1.0).abs() < 1e-5, "reference weathering was {w}");
    }

    #[test]
    fn warming_speeds_weathering_up_sharply() {
        let cool = weathering(MODERN, REFERENCE_TEMP_C - 10.0, REFERENCE_LAND, 1.0);
        let warm = weathering(MODERN, REFERENCE_TEMP_C + 10.0, REFERENCE_LAND, 1.0);
        assert!(cool < 0.6, "ten degrees colder weathered at {cool:.2}");
        assert!(warm > 1.7, "ten degrees warmer weathered at {warm:.2}");
        // And it e-folds on the measured scale.
        let step = weathering(
            MODERN,
            REFERENCE_TEMP_C + TEMPERATURE_EFOLD,
            REFERENCE_LAND,
            1.0,
        );
        assert!(
            (step - std::f32::consts::E).abs() < 0.01,
            "e-fold gave {step}"
        );
    }

    #[test]
    fn rain_and_land_both_matter() {
        let dry = weathering(MODERN, REFERENCE_TEMP_C, REFERENCE_LAND, 0.3);
        let wet = weathering(MODERN, REFERENCE_TEMP_C, REFERENCE_LAND, 2.0);
        assert!(dry < 1.0 && wet > 1.0, "{dry:.2} then {wet:.2}");

        let small = weathering(MODERN, REFERENCE_TEMP_C, 0.1, 1.0);
        let large = weathering(MODERN, REFERENCE_TEMP_C, 0.6, 1.0);
        assert!(large > small * 3.0, "{small:.2} against {large:.2}");
    }

    #[test]
    fn a_waterworld_has_no_thermostat() {
        assert_eq!(weathering(MODERN, 40.0, 0.0, 1.0), 0.0);
        // And its carbon dioxide accumulates without bound rather than settling.
        let mut co2 = MODERN;
        for _ in 0..40 {
            co2 = relax(co2, 1.0, 0.0, 1.0);
        }
        assert!(co2 > MODERN * 10.0, "it settled at {co2:.0} ppm");
    }

    #[test]
    fn carbon_settles_where_supply_meets_demand() {
        let mut co2 = MODERN;
        let supply = 2.0;
        for _ in 0..200 {
            let demand = weathering(co2, REFERENCE_TEMP_C, REFERENCE_LAND, 1.0);
            co2 = relax(co2, supply, demand, 0.5);
        }
        let balance = weathering(co2, REFERENCE_TEMP_C, REFERENCE_LAND, 1.0);
        assert!(
            (balance - supply).abs() < 0.02,
            "at {co2:.0} ppm the rates were {balance:.3} against {supply:.3}"
        );
        assert!(co2 > MODERN, "twice the volcanism should mean more carbon");
    }

    #[test]
    fn a_long_step_moves_a_long_way_without_overshooting() {
        // The reservoir responds in well under a megayear and the tectonic step is
        // megayears, so a naive forward step would fly past the answer and ring. This
        // one lands short and converges from below, which is the safe direction: the
        // inversion assumes the coupled elasticity, and at fixed temperature the true
        // one is smaller, so a single step deliberately under-reaches.
        let rate = |co2| weathering(co2, REFERENCE_TEMP_C, REFERENCE_LAND, 1.0);
        let after = relax(MODERN, 3.0, rate(MODERN), 50.0);
        assert!(
            after > MODERN * 3.0,
            "one step moved only to {after:.0} ppm"
        );
        assert!(
            rate(after) <= 3.0,
            "it overshot: weathering reached {:.2} against a supply of 3.0",
            rate(after)
        );

        // And repeating gets there.
        let mut co2 = MODERN;
        for _ in 0..60 {
            co2 = relax(co2, 3.0, rate(co2), 5.0);
        }
        assert!(
            (rate(co2) - 3.0).abs() < 0.02,
            "after sixty steps it sat at {co2:.0} ppm, weathering {:.3}",
            rate(co2)
        );
    }

    #[test]
    fn outgassing_follows_the_plate_boundaries() {
        use sim_core::{Domain, WorldSeed};
        let rate = |plates: usize| {
            let mut rng = WorldSeed::from_u128(0x11).stream(Domain::Terrain, 0, 0);
            let mut planet = Lithosphere::genesis(4, plates, 0.42, &mut rng);
            planet.step_myr(2.0, &mut rng);
            outgassing(&planet)
        };
        let ordinary = rate(9);
        assert!(
            (0.3..3.0).contains(&ordinary),
            "an ordinary planet outgassed at {ordinary:.2} times the modern rate"
        );
        // More plates is more boundary is more volcanism, which is the whole of the
        // supply side. A planet whose plates have welded into a few large ones has less
        // of it, and cools.
        assert!(
            rate(16) > rate(3),
            "sixteen plates outgassed no more than three"
        );
    }
}
