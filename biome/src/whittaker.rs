//! Reading a biome off the climate.
//!
//! Whittaker's insight, and it is one of the tidiest results in ecology: plot the world's
//! plant communities against mean annual temperature and annual precipitation and they do
//! not scatter — they fall into contiguous regions with recognisable boundaries. Rainforest
//! is what grows where it is hot and wet. Desert is what grows where it is dry, at any
//! temperature. Tundra is what grows where it is too cold for trees regardless of rain.
//!
//! So a biome is not a thing to be stored. It is a *reading* of two numbers the climate
//! already computes, which means biomes move on their own: an orogeny casts a rain shadow
//! and the forest behind it becomes steppe; a glaciation pushes taiga a thousand
//! kilometres towards the equator; a continent drifts into the subtropics and grows a
//! desert down its middle. Nobody edits a biome map, because there is no biome map.
//!
//! Two adjustments to the plain scheme, both because a diagram drawn from field sites has
//! assumptions baked into it that a whole planet does not share:
//!
//! - **Aridity is a ratio, not an amount.** Six hundred millimetres is generous in Siberia
//!   and a desert in the Sahara, because what matters is rain against how fast it
//!   evaporates. The dry boundaries here are drawn on the **aridity index** — rainfall over
//!   potential evaporation — which is what the UN uses to draw the world's drylands, and
//!   the thresholds are theirs.
//! - **Trees have a hard cold limit.** Around six or seven degrees of warmest-month
//!   temperature, wood stops being viable, and that line — not rainfall — is what puts the
//!   northern edge on the boreal forest.
//!
//! Both of those want the *warm season* rather than the annual mean, which a model with no
//! seasons does not have. [`seasonal_swing_c`] stands in for it, and it is not a fudge:
//! seasonal range is small in the tropics where the sun barely moves through the year,
//! large towards the poles where it moves enormously, and much larger inland than at the
//! coast because water has the heat capacity to ride a year out. That last term is what
//! separates Yakutsk from Reykjavík — the same annual mean, a forest at one and tundra at
//! the other, because only one of them gets a summer.

/// What grows here.
///
/// Ordered roughly from cold to hot and dry to wet, which is what the viewer's palette
/// leans on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Biome {
    /// Permanent ice. Nothing grows.
    Glacier = 0,
    /// Too cold for trees: moss, lichen, dwarf shrubs.
    Tundra = 1,
    /// The boreal forest. Conifers, and the largest forest on Earth.
    Taiga = 2,
    /// Cold and dry enough that not even taiga holds: the Gobi, the high steppe.
    ColdDesert = 3,
    /// Mid-latitude grass: prairie, pampas, steppe.
    Grassland = 4,
    /// Broadleaf and mixed forest, with a real winter.
    TemperateForest = 5,
    /// Mild, wet, and never freezing: the Pacific Northwest, Chile, Tasmania.
    TemperateRainforest = 6,
    /// Summer-dry scrub: chaparral, maquis, fynbos.
    Shrubland = 7,
    /// Hot and dry. The subtropical belt, and the lee of anything large.
    Desert = 8,
    /// Grass with scattered trees, on a strong wet season.
    Savanna = 9,
    /// Wet enough for closed canopy for part of the year.
    SeasonalForest = 10,
    /// Hot, wet all year, and the most productive thing on land.
    Rainforest = 11,
    /// Under water, on the continental shelf: light reaches the bottom.
    Shelf = 12,
    /// Under water, over ocean floor: the open sea.
    Pelagic = 13,
    /// Sea covered by ice for most of the year.
    SeaIce = 14,
}

impl Biome {
    pub const COUNT: usize = 15;

    pub const fn label(self) -> &'static str {
        match self {
            Biome::Glacier => "glacier",
            Biome::Tundra => "tundra",
            Biome::Taiga => "taiga",
            Biome::ColdDesert => "cold desert",
            Biome::Grassland => "grassland",
            Biome::TemperateForest => "temperate forest",
            Biome::TemperateRainforest => "temperate rainforest",
            Biome::Shrubland => "shrubland",
            Biome::Desert => "desert",
            Biome::Savanna => "savanna",
            Biome::SeasonalForest => "seasonal forest",
            Biome::Rainforest => "rainforest",
            Biome::Shelf => "shelf sea",
            Biome::Pelagic => "open ocean",
            Biome::SeaIce => "sea ice",
        }
    }

    pub const fn is_marine(self) -> bool {
        matches!(self, Biome::Shelf | Biome::Pelagic | Biome::SeaIce)
    }

    /// Whether the dominant plants here are trees.
    pub const fn is_forest(self) -> bool {
        matches!(
            self,
            Biome::Taiga
                | Biome::TemperateForest
                | Biome::TemperateRainforest
                | Biome::SeasonalForest
                | Biome::Rainforest
        )
    }

    /// Whether this is somewhere water, rather than heat, is the limit.
    pub const fn is_arid(self) -> bool {
        matches!(self, Biome::Desert | Biome::ColdDesert)
    }

    pub const fn from_index(index: u8) -> Biome {
        match index {
            0 => Biome::Glacier,
            1 => Biome::Tundra,
            2 => Biome::Taiga,
            3 => Biome::ColdDesert,
            4 => Biome::Grassland,
            5 => Biome::TemperateForest,
            6 => Biome::TemperateRainforest,
            7 => Biome::Shrubland,
            8 => Biome::Desert,
            9 => Biome::Savanna,
            10 => Biome::SeasonalForest,
            11 => Biome::Rainforest,
            12 => Biome::Shelf,
            13 => Biome::Pelagic,
            _ => Biome::SeaIce,
        }
    }
}

/// How deep the water has to be before a shelf becomes open ocean, in metres.
///
/// Light runs out somewhere around two hundred metres, which is what makes the shelf a
/// different world from the abyss: things can grow on the bottom.
pub const SHELF_DEPTH_M: f32 = 200.0;

/// Where wood stops working, as a warmest-month temperature in °C.
const TREE_LINE_C: f32 = 6.5;

/// Potential evaporation over a year, in millimetres, from the warmest month.
///
/// Driven by the warm season rather than the annual mean, because nearly all of a year's
/// evaporation happens in it. That is why extreme continental interiors are dry despite
/// looking mild on paper: Ulaanbaatar averages half a degree over the year and still
/// bakes at seventeen in July, and it is the July figure that empties the ground.
pub fn potential_evaporation_mm(warmest_c: f32) -> f32 {
    60.0 + 30.0 * warmest_c.max(0.0)
}

/// Rainfall against potential evaporation — the aridity index.
///
/// Below about a fifth is arid, a fifth to a half semi-arid, and past about two thirds
/// humid. These are the thresholds the UN uses to draw the world's drylands.
pub fn aridity_index(rain_mm: f32, warmest_c: f32) -> f32 {
    rain_mm / potential_evaporation_mm(warmest_c)
}

/// What lives here, from the climate and what is underneath it.
///
/// `warmest_c` is the mean temperature of the warmest month — see [`warmest_month_c`].
/// It carries most of the weight: it decides whether trees are possible at all, and it
/// sets how fast water leaves, which decides everything else.
pub fn classify(
    mean_c: f32,
    warmest_c: f32,
    rain_mm: f32,
    under_water: bool,
    depth_m: f32,
) -> Biome {
    if under_water {
        return if mean_c < -1.8 {
            // Salt water freezes a little below zero, and sea ice is its own world:
            // light barely gets through, and what lives there lives at the edge.
            Biome::SeaIce
        } else if depth_m < SHELF_DEPTH_M {
            Biome::Shelf
        } else {
            Biome::Pelagic
        };
    }

    if mean_c < -12.0 {
        // Cold enough that ice accumulates faster than it melts, whatever falls.
        return Biome::Glacier;
    }

    let arid = aridity_index(rain_mm, warmest_c);

    // Too cold for wood. Everything here is herbaceous whatever the rain, and where even
    // that cannot be supported it is polar desert — of which there is a great deal, since
    // cold air carries almost no water.
    if warmest_c < TREE_LINE_C {
        return if arid < 0.35 {
            Biome::ColdDesert
        } else {
            Biome::Tundra
        };
    }

    if arid < 0.20 {
        return if mean_c < 10.0 {
            Biome::ColdDesert
        } else {
            Biome::Desert
        };
    }

    if mean_c < 3.0 {
        // Cold and forested, or cold and open. Evaporation is slow here and frozen
        // ground holds what falls at the surface, so the boreal forest gets by on
        // rainfall that would be desert anywhere warmer.
        return if arid < 0.55 {
            Biome::ColdDesert
        } else {
            Biome::Taiga
        };
    }

    if mean_c < 12.0 {
        return if arid < 0.75 {
            Biome::Grassland
        } else if rain_mm > 1800.0 {
            Biome::TemperateRainforest
        } else {
            Biome::TemperateForest
        };
    }

    if mean_c < 20.0 {
        return if arid < 0.60 {
            // Mild and summer-dry: the Mediterranean condition, and it grows scrub
            // rather than either grass or trees.
            Biome::Shrubland
        } else if arid < 0.90 {
            Biome::Grassland
        } else if rain_mm > 2200.0 {
            Biome::TemperateRainforest
        } else {
            Biome::TemperateForest
        };
    }

    // Warm, and from here the whole answer is how much water there is against how fast
    // the heat takes it away.
    if arid < 1.2 {
        Biome::Savanna
    } else if rain_mm < 2000.0 {
        Biome::SeasonalForest
    } else {
        Biome::Rainforest
    }
}

/// How far apart the warmest and coldest months are, at a latitude and a distance from
/// the sea.
///
/// The model has no seasons, so this stands in for them, and both terms are large.
/// Seasonal range rises faster than linearly with latitude because it is driven by how
/// much the sun's height changes over a year, and that accelerates polewards. And it is
/// far larger inland than at the coast, because water has the heat capacity to ride a
/// year out: the range at sixty degrees is a few degrees in Iceland and sixty in Siberia,
/// and which of those a place gets decides whether it can grow a forest.
///
/// `continentality` runs from nought at the coast to one deep in an interior.
pub fn seasonal_swing_c(latitude_deg: f64, continentality: f32) -> f32 {
    let reach = (latitude_deg.abs() / 90.0).clamp(0.0, 1.0) as f32;
    let by_latitude = 2.0 + 40.0 * reach.powf(1.6);
    by_latitude * (0.5 + continentality.clamp(0.0, 1.0))
}

/// The warmest month, given an annual mean and where the place is.
pub fn warmest_month_c(mean_c: f32, latitude_deg: f64, continentality: f32) -> f32 {
    mean_c + seasonal_swing_c(latitude_deg, continentality) / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Somewhere on land, with the warm season worked out from where it is.
    fn on_land(mean_c: f32, rain_mm: f32, latitude: f64, continentality: f32) -> Biome {
        let warmest = warmest_month_c(mean_c, latitude, continentality);
        classify(mean_c, warmest, rain_mm, false, 0.0)
    }

    #[test]
    fn the_familiar_places_come_out_right() {
        // Each of these is a real place with real numbers, and the classification has to
        // recognise it. This is the whole test of the scheme.
        //
        // Deliberately not on the list: Ulaanbaatar, Kansas, Nairobi. Each sits on a
        // genuine gradient — steppe against taiga, prairie against woodland, savanna
        // against dry forest — where the real boundary is set by fire, grazing and the
        // shape of the dry season rather than by annual totals. Pinning them would be
        // fitting the scheme to a coin toss.
        let cases: [(&str, f32, f32, f64, f32, Biome); 10] = [
            ("Manaus", 27.0, 2300.0, -3.0, 0.6, Biome::Rainforest),
            ("Darwin", 27.5, 1700.0, -12.0, 0.3, Biome::SeasonalForest),
            ("Serengeti", 21.0, 800.0, -2.0, 0.5, Biome::Savanna),
            ("Cairo", 22.0, 25.0, 30.0, 0.8, Biome::Desert),
            ("Athens", 18.5, 400.0, 38.0, 0.2, Biome::Shrubland),
            ("Paris", 11.5, 640.0, 49.0, 0.35, Biome::TemperateForest),
            ("Denver", 10.5, 400.0, 40.0, 0.9, Biome::Grassland),
            (
                "Bergen",
                8.0,
                2250.0,
                60.0,
                0.05,
                Biome::TemperateRainforest,
            ),
            ("Yakutsk", -8.0, 240.0, 62.0, 1.0, Biome::Taiga),
            ("Barrow", -11.0, 110.0, 71.0, 0.0, Biome::Tundra),
        ];
        for (place, mean, rain, latitude, cont, wanted) in cases {
            let got = on_land(mean, rain, latitude, cont);
            assert_eq!(got, wanted, "{place} came out as {}", got.label());
        }
    }

    #[test]
    fn dryness_is_relative_to_the_heat() {
        // Six hundred millimetres is a forest in Siberia and scrub in the Sahel. A
        // scheme with a fixed rainfall threshold cannot say both, and it is the single
        // most important correction to make to the plain diagram.
        let cold = on_land(-2.0, 600.0, 60.0, 0.6);
        let hot = on_land(30.0, 600.0, 10.0, 0.6);
        assert!(
            cold.is_forest(),
            "cold and damp came out as {}",
            cold.label()
        );
        assert!(!hot.is_forest(), "hot and dry came out as {}", hot.label());
    }

    #[test]
    fn the_tree_line_is_about_summer_not_the_year() {
        // Two places with the same annual mean and the same rain: one maritime with mild
        // seasons, one continental with fierce ones. Only the second can grow a forest,
        // because only its summer gets warm enough for wood. This is why the boreal
        // forest reaches so much further north in Siberia than in Iceland.
        let mean = -3.0;
        let rain = 500.0;
        let maritime = classify(mean, mean + 2.0, rain, false, 0.0);
        let continental = classify(mean, mean + 14.0, rain, false, 0.0);
        assert_eq!(maritime, Biome::Tundra);
        assert_eq!(continental, Biome::Taiga);
    }

    #[test]
    fn seasons_are_mild_in_the_tropics_fierce_at_the_poles_and_fiercer_inland() {
        assert!(seasonal_swing_c(0.0, 0.5) < 4.0);
        assert!(seasonal_swing_c(70.0, 0.5) > 20.0);
        // Iceland against Siberia at the same latitude.
        assert!(seasonal_swing_c(62.0, 1.0) > seasonal_swing_c(62.0, 0.0) * 2.5);
    }

    #[test]
    fn potential_evaporation_follows_the_warm_season() {
        assert!(potential_evaporation_mm(30.0) > potential_evaporation_mm(10.0) * 2.0);
        // And it does not go negative or to nothing when the warm season is below zero.
        assert!(potential_evaporation_mm(-40.0) > 0.0);
        assert_eq!(
            potential_evaporation_mm(-40.0),
            potential_evaporation_mm(0.0)
        );
    }

    #[test]
    fn under_water_it_is_about_depth_and_ice() {
        assert_eq!(classify(18.0, 20.0, 0.0, true, 60.0), Biome::Shelf);
        assert_eq!(classify(4.0, 6.0, 0.0, true, 4000.0), Biome::Pelagic);
        assert_eq!(classify(-6.0, -4.0, 0.0, true, 4000.0), Biome::SeaIce);
        // And salt water does not freeze at zero.
        assert_eq!(classify(-1.0, 0.0, 0.0, true, 4000.0), Biome::Pelagic);
    }

    #[test]
    fn a_cold_enough_place_is_ice_whatever_falls_on_it() {
        for rain in [0.0, 400.0, 3000.0] {
            assert_eq!(on_land(-25.0, rain, 80.0, 0.5), Biome::Glacier);
        }
    }

    #[test]
    fn every_biome_is_reachable_from_some_climate() {
        // A classification with an unreachable case is a classification with a bug in it,
        // and the swept region is the one a planet actually visits.
        let mut seen = std::collections::BTreeSet::new();
        for t in -40..=45 {
            for r in 0..=40 {
                let mean = t as f32;
                let rain = r as f32 * 100.0;
                for latitude in [0.0, 25.0, 45.0, 65.0, 85.0] {
                    for cont in [0.0, 0.5, 1.0] {
                        seen.insert(on_land(mean, rain, latitude, cont));
                    }
                }
                for depth in [50.0, 4000.0] {
                    seen.insert(classify(mean, mean + 3.0, rain, true, depth));
                }
            }
        }
        for index in 0..Biome::COUNT as u8 {
            let biome = Biome::from_index(index);
            assert!(seen.contains(&biome), "nothing is ever {}", biome.label());
        }
    }

    #[test]
    fn the_boundaries_do_not_jump_about() {
        // Walking a step at a time across the whole space should not flicker: a small
        // change in climate should mean the same biome or an adjacent one, not a
        // different answer every step. Counting the changes is a cheap way to catch a
        // threshold written the wrong way round.
        let mut flips = 0;
        for t in -30..45 {
            let mut last = on_land(t as f32, 0.0, 40.0, 0.5);
            for r in 1..=40 {
                let now = on_land(t as f32, r as f32 * 100.0, 40.0, 0.5);
                if now != last {
                    flips += 1;
                    last = now;
                }
            }
        }
        // Seventy-five temperature rows, each crossing at most a handful of boundaries
        // on the way from bone dry to soaking.
        assert!(flips < 250, "the classification flipped {flips} times");
    }

    #[test]
    fn indices_round_trip() {
        for index in 0..Biome::COUNT as u8 {
            assert_eq!(Biome::from_index(index) as u8, index);
        }
    }
}
