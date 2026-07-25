//! How much the biosphere actually makes, in grams of dry matter per square metre a year.
//!
//! The biome says what kind of thing grows; this says how much of it there is. They are
//! different questions and the second is the one everything downstream needs — how many
//! animals a region can carry, how much carbon the land holds, whether people can farm
//! there — so it is worth having a number rather than a label.
//!
//! On land this is the **Miami model**: net primary production limited independently by
//! temperature and by rainfall, and whichever limit bites harder is the answer. It is
//! about as simple as an empirical productivity model gets, it was fitted to field
//! measurements from every continent in the 1970s, and it holds up remarkably well —
//! which is unsurprising, because the thing it captures is that a plant needs both warmth
//! and water and cannot trade one for the other.
//!
//! At sea the limit is different and the honest answer is thinner. What bounds ocean
//! productivity is nutrient supply, and nutrients come from rivers and from upwelling —
//! neither of which is modelled yet. What is here uses light and temperature, with the
//! shelf standing in for river-fed nutrients, and it should be read as a placeholder that
//! puts productivity roughly where it belongs rather than as a nutrient budget.

use crate::whittaker::Biome;

/// The Miami model's ceiling, in grams of dry matter per square metre per year.
///
/// Nothing on land exceeds about three kilograms a square metre a year, and the wettest
/// tropical forest is what approaches it.
const CEILING: f32 = 3000.0;

/// Net primary production on land, from temperature and rainfall.
pub fn on_land(mean_c: f32, rain_mm: f32) -> f32 {
    // Temperature limit: a logistic that is near zero below freezing and saturates in
    // the high twenties.
    let by_heat = CEILING / (1.0 + (1.315 - 0.119 * mean_c).exp());
    // Water limit: saturating, because past a point more rain adds nothing — the plants
    // are already using all the light there is.
    let by_water = CEILING * (1.0 - (-0.000_664 * rain_mm).exp());
    by_heat.min(by_water).max(0.0)
}

/// Net primary production at sea, per square metre of surface.
///
/// An order of magnitude below a forest per unit area, which is right: the ocean and the
/// land fix roughly the same amount of carbon a year, and the ocean is two and a half
/// times the area with all of its production in a thin sunlit layer.
pub fn at_sea(mean_c: f32, sunlight: f32, shelf: bool, frozen: bool) -> f32 {
    if frozen {
        // Under ice there is very little light and very little mixing. Not nothing —
        // the ice edge is one of the most productive places on the planet — but that
        // belongs to the edge rather than to the middle.
        return 12.0;
    }
    // Warm water is more productive per unit of light up to a point, then stratifies and
    // starves itself of nutrients from below, which is why the tropical open ocean is a
    // desert. The peak is around the temperate latitudes.
    let by_heat = (-((mean_c - 12.0) / 16.0).powi(2)).exp();
    let by_light = (sunlight / 400.0).clamp(0.0, 1.2);
    // A shelf is fed by rivers and stirred by the bottom; the open ocean is neither.
    let feeding = if shelf { 1.0 } else { 0.28 };
    360.0 * by_heat * by_light * feeding
}

/// Production for a cell, whichever kind of place it is.
pub fn of(biome: Biome, mean_c: f32, rain_mm: f32, sunlight: f32, shelf: bool) -> f32 {
    match biome {
        Biome::Glacier => 0.0,
        Biome::SeaIce => at_sea(mean_c, sunlight, shelf, true),
        Biome::Shelf | Biome::Pelagic => at_sea(mean_c, sunlight, shelf, false),
        _ => on_land(mean_c, rain_mm),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rainforest_is_the_most_productive_thing_on_land() {
        let rainforest = on_land(27.0, 2500.0);
        assert!(
            (2000.0..3000.0).contains(&rainforest),
            "a rainforest made {rainforest:.0} g/m²/yr"
        );
        for (name, t, r) in [
            ("temperate forest", 11.0, 800.0),
            ("savanna", 25.0, 900.0),
            ("taiga", -2.0, 450.0),
            ("desert", 24.0, 60.0),
        ] {
            let there = on_land(t, r);
            assert!(there < rainforest, "{name} beat the rainforest");
        }
    }

    #[test]
    fn the_numbers_match_what_is_measured() {
        // The Miami model's whole claim is that these come out near the field figures,
        // so the field figures are the test.
        let cases: [(&str, f32, f32, f32, f32); 5] = [
            ("tropical rainforest", 27.0, 2500.0, 2000.0, 3000.0),
            ("temperate forest", 11.0, 900.0, 800.0, 1600.0),
            ("grassland", 13.0, 500.0, 500.0, 1000.0),
            ("boreal forest", -2.0, 450.0, 200.0, 700.0),
            ("desert", 24.0, 50.0, 0.0, 200.0),
        ];
        for (name, t, r, low, high) in cases {
            let got = on_land(t, r);
            assert!(
                (low..high).contains(&got),
                "{name} made {got:.0} g/m²/yr, expected {low:.0} to {high:.0}"
            );
        }
    }

    #[test]
    fn whichever_limit_bites_harder_is_the_answer() {
        // A hot desert is not productive however hot it is, and a soaking tundra is not
        // productive however wet. Neither limit can be bought off with the other.
        let hot_and_dry = on_land(30.0, 40.0);
        let wet_and_cold = on_land(-8.0, 2000.0);
        assert!(hot_and_dry < 150.0, "a hot desert made {hot_and_dry:.0}");
        // The Miami model runs a little generous in the cold — measured tundra is nearer
        // a hundred and fifty — which is a known bias of it and not worth correcting
        // with a coefficient of my own.
        assert!(wet_and_cold < 320.0, "a frozen bog made {wet_and_cold:.0}");
    }

    #[test]
    fn more_of_either_never_makes_less() {
        let mut last = 0.0;
        for t in -20..40 {
            let now = on_land(t as f32, 5000.0);
            assert!(now >= last, "productivity fell as it warmed at {t} °C");
            last = now;
        }
        let mut last = 0.0;
        for r in 0..50 {
            let now = on_land(25.0, r as f32 * 100.0);
            assert!(now >= last, "productivity fell as it got wetter");
            last = now;
        }
    }

    #[test]
    fn nothing_grows_on_a_glacier() {
        assert_eq!(of(Biome::Glacier, -30.0, 500.0, 200.0, false), 0.0);
    }

    #[test]
    fn a_shelf_out_produces_the_open_ocean() {
        let shelf = at_sea(12.0, 350.0, true, false);
        let open = at_sea(12.0, 350.0, false, false);
        assert!(
            shelf > open * 2.0,
            "shelf {shelf:.0} against open {open:.0}"
        );
    }

    #[test]
    fn the_tropical_open_ocean_is_a_desert() {
        // One of the more counter-intuitive facts about the sea, and a real one: the
        // warm open ocean stratifies, nothing brings nutrients up from below, and it is
        // among the least productive water on the planet despite all the light.
        let tropics = at_sea(28.0, 420.0, false, false);
        let temperate = at_sea(12.0, 300.0, false, false);
        assert!(
            temperate > tropics,
            "the tropics made {tropics:.0} and the mid-latitudes {temperate:.0}"
        );
    }

    #[test]
    fn the_sea_makes_far_less_per_square_metre_than_a_forest() {
        let best_sea = at_sea(12.0, 400.0, true, false);
        let forest = on_land(20.0, 1500.0);
        assert!(
            forest > best_sea * 3.0,
            "sea {best_sea:.0} against forest {forest:.0}"
        );
    }

    #[test]
    fn under_ice_there_is_almost_nothing() {
        assert!(at_sea(-5.0, 100.0, false, true) < 30.0);
    }
}
