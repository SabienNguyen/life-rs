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
//! At sea the limit is different, and it is the one that used to be missing. What bounds
//! ocean productivity is **nutrient supply** — not light and not warmth, which is why the
//! sunlit tropical ocean is one of the emptiest places on the planet. Everything that
//! grows in the sunlit layer sinks when it dies, so the surface is stripped and the depths
//! are rich, and the productive sea is wherever deep water is being brought back up. That
//! supply comes from `ocean`, and this crate multiplies by it.

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
///
/// The limiting term is `nutrients`, and that is the correction the `ocean` crate exists
/// to make. This used to carry a flat multiplier — one on the shelf, a bit over a quarter
/// everywhere else — with a comment admitting it was a placeholder for a nutrient budget
/// that did not exist. It got the *pattern* roughly right by accident, because shelves
/// really are better fed, and it got the reason wrong, which meant it could not produce
/// any of the consequences: no eastern-boundary fisheries, no equatorial cold tongue, no
/// difference between a shelf below a rainforest and a shelf below a desert.
pub fn at_sea(mean_c: f32, sunlight: f32, shelf: bool, frozen: bool, nutrients: f32) -> f32 {
    if frozen {
        // Under ice there is very little light and very little mixing. Not nothing —
        // the ice edge is one of the most productive places on the planet — but that
        // belongs to the edge rather than to the middle.
        return 12.0;
    }
    // Water below its freezing point is not water. Seawater freezes near −1.8 °C, and
    // there is no smooth roll-off to be had below that — either the cell is liquid or it
    // is the ice case above.
    if mean_c <= -1.8 {
        return 0.0;
    }
    // What warmth does on its own, which is **monotone and weak**. The old form peaked in
    // the mid-latitudes and fell away in the tropics, which put the tropical ocean's
    // emptiness in the temperature term — where it does not belong. It is stratification,
    // and stratification is now in the nutrient supply.
    //
    // Weak because the strong exponential everyone quotes — Eppley's, a doubling per ten
    // degrees — bounds the *maximum* growth rate of a cell that has everything it needs.
    // Annual production in a nutrient-limited ocean is nothing like that sensitive to
    // temperature, and using Eppley directly made a 28 °C gyre out-produce temperate water
    // by half again despite being starved. What is left here is the residual: about a
    // third more production across the whole liquid range, normalised at fifteen degrees.
    let by_heat = (0.025 * (mean_c - 15.0)).exp().min(1.5);
    let by_light = (sunlight / 400.0).clamp(0.0, 1.2);
    let by_food = ocean::nutrients::realised(nutrients);
    // A shelf still gets a little for being a shelf, over and above what its rivers bring:
    // the bottom is inside the mixed layer, so what sinks is stirred back up rather than
    // lost to the deep.
    let benthic = if shelf { 1.25 } else { 1.0 };
    620.0 * by_heat * by_light * by_food * benthic
}

/// Production for a cell, whichever kind of place it is.
pub fn of(
    biome: Biome,
    mean_c: f32,
    rain_mm: f32,
    sunlight: f32,
    shelf: bool,
    nutrients: f32,
) -> f32 {
    match biome {
        Biome::Glacier => 0.0,
        Biome::SeaIce => at_sea(mean_c, sunlight, shelf, true, nutrients),
        Biome::Shelf | Biome::Pelagic => at_sea(mean_c, sunlight, shelf, false, nutrients),
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
        assert_eq!(of(Biome::Glacier, -30.0, 500.0, 200.0, false, 0.5), 0.0);
    }

    #[test]
    fn a_well_fed_shelf_out_produces_a_starved_open_ocean() {
        // Same water, same light. What separates them is what is in it — which is now
        // where the difference actually comes from rather than a flag standing in for it.
        let shelf = at_sea(12.0, 350.0, true, false, 0.9);
        let open = at_sea(12.0, 350.0, false, false, 0.1);
        assert!(shelf > open * 2.0, "shelf {shelf:.0} against open {open:.0}");
    }

    #[test]
    fn feeding_the_open_ocean_makes_it_produce() {
        // The other side of the same claim, and the one that matters: an upwelling zone
        // in open water out-produces a shelf that nothing feeds. Peru is not a shelf sea.
        let upwelling = at_sea(16.0, 380.0, false, false, 0.95);
        let barren_shelf = at_sea(16.0, 380.0, true, false, 0.05);
        assert!(
            upwelling > barren_shelf,
            "an upwelling zone made {upwelling:.0} against {barren_shelf:.0} on dead shelf"
        );
    }

    #[test]
    fn the_tropical_open_ocean_is_a_desert() {
        // One of the more counter-intuitive facts about the sea, and a real one: the warm
        // open ocean stratifies, nothing brings nutrients up from below, and it is among
        // the least productive water on the planet despite having all the light. The
        // stratification now lives in the nutrient supply, so the two cells differ in what
        // they are fed as well as in how warm they are — which is the point.
        let tropics = at_sea(28.0, 420.0, false, false, 0.05);
        let temperate = at_sea(12.0, 300.0, false, false, 0.55);
        assert!(
            temperate > tropics * 1.5,
            "the tropics made {tropics:.0} and the mid-latitudes {temperate:.0}"
        );
    }

    #[test]
    fn food_is_what_limits_the_sea() {
        // Monotone in nutrients, and steeply so at the bottom of the range: the whole
        // claim of this crate’s marine half.
        let at = |n| at_sea(14.0, 350.0, false, false, n);
        let mut last = f32::MIN;
        for i in 0..=20 {
            let now = at(i as f32 / 20.0);
            assert!(now >= last, "production fell as food increased");
            last = now;
        }
        assert!(at(1.0) > at(0.0) * 4.0, "food barely mattered");
    }

    #[test]
    fn the_sea_makes_far_less_per_square_metre_than_a_forest() {
        let best_sea = at_sea(12.0, 400.0, true, false, 1.0);
        let forest = on_land(20.0, 1500.0);
        assert!(
            forest > best_sea * 2.0,
            "sea {best_sea:.0} against forest {forest:.0}"
        );
    }

    #[test]
    fn the_richest_sea_is_in_the_range_a_rich_sea_is_measured_at() {
        // An upwelling zone fixes three hundred to five hundred grams a square metre a
        // year, which is a wet grassland rather than a forest. A model that puts it at a
        // forest’s rate has the ocean carrying more carbon than it does.
        let peru = at_sea(16.0, 380.0, true, false, 1.0);
        assert!(
            (250.0..700.0).contains(&peru),
            "the richest sea made {peru:.0} g/m²/yr"
        );
    }

    #[test]
    fn under_ice_there_is_almost_nothing() {
        assert!(at_sea(-5.0, 100.0, false, true, 0.8) < 30.0);
    }
}
