//! What the sunlit layer has to eat.
//!
//! The single fact that decides where the ocean is alive: **everything that grows in the
//! sunlit layer sinks when it dies**. Nitrate and phosphate leave the surface with the
//! bodies and are remineralised in the dark, so the top hundred metres of an undisturbed
//! ocean is stripped bare and the water below it is rich. A tropical ocean under a
//! vertical sun with a warm, strongly stratified surface is therefore a desert, and it
//! looks like one from orbit.
//!
//! Three things break that stratification and put the nutrients back where the light is:
//!
//! - **Upwelling**, which is the big one, and which `lib.rs` computes from wind and
//!   geometry.
//! - **Winter mixing**, which is why the high-latitude ocean has a spring bloom and the
//!   tropics do not: cold surface water is dense, the density contrast with the water
//!   below is small, and a winter storm turns the column over.
//! - **Rivers**, which is why continental shelves are productive out of all proportion to
//!   their area. Runoff carries what the land has weathered, so a shelf below a wet
//!   continent is fed and a shelf below a desert is not.
//!
//! All three are here. What is not is any actual budget — no nitrate inventory, no
//! remineralisation depth, no denitrification. This is a supply *index* between nought and
//! one, and everything downstream should treat it as one.

use climate::Climate;
use geo::{CellId, Lithosphere};

/// The depth at which a cell stops counting as shelf, in metres.
///
/// Two hundred, which is the usual definition and is also roughly where the bottom stops
/// being stirred by anything that happens at the surface.
pub const SHELF_DEPTH_M: f32 = 200.0;

/// Temperature above which the surface layer is strongly stratified, in °C.
///
/// Warm water is buoyant and stays on top; that is the whole of it. Twenty-two degrees is
/// about where the tropical ocean's permanent thermocline shuts off exchange with the
/// deep for good.
const STRATIFIED_C: f32 = 22.0;
/// Temperature below which the column turns over freely every winter.
const MIXED_C: f32 = 6.0;

/// How much of a cell's nutrient supply a strong upwelling can provide on its own.
const FROM_UPWELLING: f32 = 0.85;
/// How much winter mixing provides where the water is cold enough for it.
const FROM_MIXING: f32 = 0.55;
/// How much river supply provides on a well-watered shelf.
const FROM_RIVERS: f32 = 0.45;
/// Rainfall, in mm/yr, at which a coast delivers all the river-borne nutrient it is going
/// to.
const AMPLE_RUNOFF_MM: f32 = 900.0;

/// The nutrient supply index for every cell, 0 to 1. Land is zero.
pub fn from_upwelling(planet: &Lithosphere, climate: &Climate, upwelling: &[f32]) -> Vec<f32> {
    let grid = planet.grid();
    let mut supply = vec![0.0f32; grid.len()];

    for cell in grid.cells() {
        if planet.is_land(cell) {
            continue;
        }
        let temperature = climate.temperature_c(cell);

        // Upwelling, damped by how hard the column is to break. Even a strong wind cannot
        // lift much through a warm tropical thermocline, which is why the equatorial
        // divergence zone is productive but not as productive as Peru.
        let stratification =
            ((temperature - MIXED_C) / (STRATIFIED_C - MIXED_C)).clamp(0.0, 1.0);
        let lifted = FROM_UPWELLING * upwelling[cell as usize] * (1.0 - 0.55 * stratification);

        // Winter mixing: free where the water is cold, absent where it is warm.
        let mixed = FROM_MIXING * (1.0 - stratification);

        // Rivers, on the shelf only, in proportion to what falls on the land next door.
        let depth = (-planet.height_above_sea_m(cell)).max(0.0);
        let fed = if depth <= SHELF_DEPTH_M {
            let mut rain = 0.0;
            let mut land = 0;
            for &n in grid.neighbours(cell) {
                if planet.is_land(n) {
                    rain += climate.rain_mm(n);
                    land += 1;
                }
            }
            if land == 0 {
                0.0
            } else {
                FROM_RIVERS * ((rain / land as f32) / AMPLE_RUNOFF_MM).clamp(0.0, 1.0)
            }
        } else {
            0.0
        };

        // The three do not simply add: they are three ways of doing the same job, and a
        // cell already well supplied gains little from a second one. Diminishing returns
        // by taking the complement of the product of what each one *fails* to supply,
        // which is the standard way to combine independent partial successes and keeps
        // the result inside the range without a clamp doing the work.
        let missing = (1.0 - lifted.clamp(0.0, 1.0))
            * (1.0 - mixed.clamp(0.0, 1.0))
            * (1.0 - fed.clamp(0.0, 1.0));
        supply[cell as usize] = (1.0 - missing).clamp(0.0, 1.0);
    }
    supply
}

/// How much of its light-and-temperature potential a patch of sea can actually realise.
///
/// The multiplier `biome` applies to marine production. Not the supply index itself: a
/// nutrient-starved ocean is not *dead*, it is oligotrophic, and the open subtropical
/// gyres do sustain a thin permanent population. The floor is what that is.
pub fn realised(supply: f32) -> f32 {
    // A saturating response rather than a straight line, because the limiting nutrient
    // saturates: past a point the plankton are light-limited instead and more nitrate buys
    // nothing. Michaelis–Menten is the usual form and this is that with the constants
    // rolled in.
    // A subtropical gyre fixes something like a twelfth of what an upwelling zone does,
    // so the floor is genuinely low and the curve genuinely steep. The first values here
    // were four times as generous and it showed downstream: a starved tropical gyre came
    // out out-producing well-fed temperate water, because a quarter of the potential is
    // not a desert.
    const FLOOR: f32 = 0.05;
    const HALF: f32 = 0.50;
    let s = supply.clamp(0.0, 1.0);
    FLOOR + (1.0 - FLOOR) * (s / (s + HALF))
}

/// Whether a cell is shallow enough to be shelf.
pub fn is_shelf(planet: &Lithosphere, cell: CellId) -> bool {
    !planet.is_land(cell) && (-planet.height_above_sea_m(cell)).max(0.0) <= SHELF_DEPTH_M
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_starved_sea_is_thin_rather_than_dead() {
        let nothing = realised(0.0);
        assert!(
            (0.05..0.2).contains(&nothing),
            "an oligotrophic gyre realised {nothing:.2} of its potential"
        );
    }

    #[test]
    fn more_food_is_never_less_production() {
        let mut previous = f32::MIN;
        for i in 0..=20 {
            let got = realised(i as f32 / 20.0);
            assert!(got >= previous, "production fell at supply {}", i as f32 / 20.0);
            previous = got;
        }
    }

    #[test]
    fn the_response_saturates() {
        // The point of Michaelis–Menten: the first half of the nutrient buys most of the
        // production, and doubling a rich supply barely moves it.
        let early = realised(0.35) - realised(0.0);
        let late = realised(1.0) - realised(0.65);
        assert!(
            early > late * 2.0,
            "the response is nearly linear: {early:.3} against {late:.3}"
        );
    }

    #[test]
    fn a_full_supply_realises_most_of_the_potential() {
        // Two thirds rather than all of it: even an upwelling zone spends part of the year
        // light-limited, and the curve saturates rather than reaching one.
        assert!(realised(1.0) > 0.6, "{}", realised(1.0));
        assert!(realised(1.0) <= 1.0);
    }

    #[test]
    fn the_richest_water_out_produces_the_poorest_by_an_order_of_magnitude() {
        // The measured contrast: a subtropical gyre fixes something like sixty grams a
        // square metre a year and an upwelling zone four hundred. If this ratio is small,
        // nutrients are not really the limiting factor and the crate is decoration.
        let ratio = realised(1.0) / realised(0.0);
        assert!(
            (8.0..20.0).contains(&ratio),
            "the richest sea was only {ratio:.1} times the poorest"
        );
    }
}
