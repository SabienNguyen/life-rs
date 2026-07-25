//! What an animal is, at the resolution deep time can see.
//!
//! Not an organism and not a genome: a set of tolerances and a body mass. Over megayears
//! the questions that matter about a species are where it can live, how much of it there
//! can be, and what eats it — and all three fall out of those few numbers through
//! relationships that are among the most robust in ecology.
//!
//! **Kleiber's law.** Metabolic demand goes as body mass to the three-quarters, not
//! linearly. A shrew burns far more per gram than an elephant does, which is why a shrew
//! must eat constantly and an elephant can afford to be picky.
//!
//! **Damuth's law.** Population density goes as mass to the *minus* three-quarters, which
//! is the same exponent from the other side: the two cancel, so a given supply of food
//! supports roughly the same total *biomass* whatever it is divided into. Ten thousand
//! rabbits and one rhinoceros weigh about the same and eat about the same, and that is
//! why the trophic pyramid is a pyramid of mass rather than of individuals.
//!
//! **The ten percent rule.** Each step up the food chain keeps about a tenth of what the
//! step below it made. This is thermodynamics rather than biology, and it is why there
//! are so few large predators and why nothing anywhere lives at the sixth trophic level.

use biome::Biome;
use sim_core::Rng;

/// Where in the food chain something feeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Trophic {
    /// Eats plants, or plankton.
    Herbivore,
    /// Eats herbivores.
    Carnivore,
}

impl Trophic {
    pub const fn label(self) -> &'static str {
        match self {
            Trophic::Herbivore => "herbivore",
            Trophic::Carnivore => "carnivore",
        }
    }
}

/// How much of what a level below produced reaches the level above.
///
/// A tenth, give or take, and it is the reason the pyramid narrows so fast.
pub const TRANSFER: f32 = 0.10;

/// What share of a year's plant growth is actually edible.
///
/// Most of what a forest makes is wood, and almost nothing eats wood. Grassland is far
/// more available to grazers than forest is, which is why the great herds are on the
/// plains rather than under the canopy — a fact this gets for free by asking the biome.
pub fn edible_share(biome: Biome) -> f32 {
    match biome {
        Biome::Glacier => 0.0,
        Biome::Rainforest | Biome::SeasonalForest | Biome::TemperateRainforest => 0.06,
        Biome::TemperateForest | Biome::Taiga => 0.09,
        Biome::Savanna | Biome::Grassland => 0.42,
        Biome::Shrubland | Biome::Tundra => 0.30,
        Biome::Desert | Biome::ColdDesert => 0.24,
        // At sea nearly all of the production is plankton, and nearly all of it is eaten.
        Biome::Shelf | Biome::Pelagic => 0.75,
        Biome::SeaIce => 0.50,
    }
}

/// One kind of animal.
#[derive(Clone, Debug)]
pub struct Species {
    pub name: String,
    pub trophic: Trophic,
    pub marine: bool,
    /// Typical adult mass, in kilograms.
    pub mass_kg: f32,
    /// The band of mean annual temperature it can live in, in °C.
    pub warmest_c: f32,
    pub coldest_c: f32,
    /// The least rain it can do with, in millimetres a year. Ignored at sea.
    pub driest_mm: f32,
    /// How readily it crosses into a neighbouring cell each megayear.
    pub dispersal: f32,
    /// When it first appeared, in megayears since the world began.
    pub arose_myr: f64,
}

impl Species {
    /// What one animal needs a year, in grams of dry matter.
    ///
    /// Kleiber's exponent, scaled so a fifty-kilogram herbivore eats roughly its own
    /// mass every fortnight — which is about what a deer does.
    pub fn demand_g_per_year(&self) -> f32 {
        const RATE: f32 = 26_000.0;
        RATE * self.mass_kg.max(0.001).powf(0.75)
    }

    /// How well this place suits it, from nought to one.
    ///
    /// A product rather than a sum: a species needs *every* one of its requirements met,
    /// and somewhere warm enough but far too dry is no use at all. Tapered rather than
    /// square-edged, because the edge of a range is a place a species is scarce rather
    /// than a line it stops at.
    pub fn suitability(&self, temp_c: f32, rain_mm: f32, biome: Biome) -> f32 {
        if biome.is_marine() != self.marine {
            return 0.0;
        }
        if biome == Biome::Glacier {
            return 0.0;
        }

        // Temperature: full marks inside the band, falling away over a few degrees.
        const TAPER_C: f32 = 5.0;
        let heat = if temp_c > self.warmest_c {
            1.0 - (temp_c - self.warmest_c) / TAPER_C
        } else if temp_c < self.coldest_c {
            1.0 - (self.coldest_c - temp_c) / TAPER_C
        } else {
            1.0
        };

        let water = if self.marine {
            1.0
        } else {
            const TAPER_MM: f32 = 250.0;
            if rain_mm < self.driest_mm {
                1.0 - (self.driest_mm - rain_mm) / TAPER_MM
            } else {
                1.0
            }
        };

        (heat.clamp(0.0, 1.0) * water.clamp(0.0, 1.0)).clamp(0.0, 1.0)
    }

    /// A species drawn at random, of a given kind.
    ///
    /// Body masses are drawn log-uniformly across four orders of magnitude, which is
    /// roughly how real body sizes are distributed — many small things and few large
    /// ones — and tolerances are a band of random width somewhere in the habitable range.
    pub fn random(trophic: Trophic, marine: bool, arose_myr: f64, rng: &mut Rng) -> Species {
        let centre = rng.range_f64(-8.0, 28.0) as f32;
        Species::around(centre, trophic, marine, arose_myr, rng)
    }

    /// A species built around a particular temperature — the one it arose at.
    pub fn around(
        centre: f32,
        trophic: Trophic,
        marine: bool,
        arose_myr: f64,
        rng: &mut Rng,
    ) -> Species {
        let mass = 10f32.powf(rng.range_f64(-1.5, 3.0) as f32);
        let width = rng.range_f64(6.0, 30.0) as f32;
        Species {
            name: String::new(),
            trophic,
            marine,
            mass_kg: mass,
            coldest_c: centre - width / 2.0,
            warmest_c: centre + width / 2.0,
            driest_mm: if marine {
                0.0
            } else {
                rng.range_f64(0.0, 700.0) as f32
            },
            dispersal: rng.range_f64(0.15, 0.9) as f32,
            arose_myr,
        }
    }

    /// How wide a range of temperature it will put up with.
    pub fn tolerance_c(&self) -> f32 {
        self.warmest_c - self.coldest_c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Domain, WorldSeed};

    fn a_species(mass: f32, coldest: f32, warmest: f32, driest: f32) -> Species {
        Species {
            name: "test".into(),
            trophic: Trophic::Herbivore,
            marine: false,
            mass_kg: mass,
            coldest_c: coldest,
            warmest_c: warmest,
            driest_mm: driest,
            dispersal: 0.5,
            arose_myr: 0.0,
        }
    }

    #[test]
    fn a_big_animal_eats_more_but_less_per_gram() {
        let mouse = a_species(0.02, 0.0, 30.0, 0.0);
        let elephant = a_species(4000.0, 0.0, 30.0, 0.0);
        assert!(elephant.demand_g_per_year() > mouse.demand_g_per_year() * 100.0);

        let per_kg = |s: &Species| s.demand_g_per_year() / s.mass_kg;
        assert!(
            per_kg(&mouse) > per_kg(&elephant) * 10.0,
            "a mouse should burn far more per gram: {:.0} against {:.0}",
            per_kg(&mouse),
            per_kg(&elephant)
        );
    }

    #[test]
    fn a_deer_eats_about_what_a_deer_eats() {
        // Fifty kilograms, and it should come out somewhere near its own body mass every
        // couple of weeks — a tonne or two a year of dry matter.
        let deer = a_species(50.0, -5.0, 25.0, 200.0);
        let tonnes = deer.demand_g_per_year() / 1.0e6;
        assert!(
            (0.3..3.0).contains(&tonnes),
            "a deer ate {tonnes:.2} tonnes a year"
        );
    }

    #[test]
    fn somewhere_has_to_meet_every_requirement_at_once() {
        let s = a_species(10.0, 5.0, 25.0, 400.0);
        // Right on both counts.
        assert_eq!(s.suitability(15.0, 900.0, Biome::Grassland), 1.0);
        // Warm enough, far too dry.
        assert_eq!(s.suitability(15.0, 100.0, Biome::Desert), 0.0);
        // Wet enough, far too cold.
        assert_eq!(s.suitability(-20.0, 900.0, Biome::Tundra), 0.0);
    }

    #[test]
    fn the_edge_of_a_range_is_a_slope_not_a_wall() {
        let s = a_species(10.0, 5.0, 25.0, 0.0);
        let inside = s.suitability(24.0, 900.0, Biome::Grassland);
        let edge = s.suitability(27.0, 900.0, Biome::Grassland);
        let outside = s.suitability(40.0, 900.0, Biome::Grassland);
        assert_eq!(inside, 1.0);
        assert!(
            (0.1..0.9).contains(&edge),
            "the edge of the range read {edge:.2}"
        );
        assert_eq!(outside, 0.0);
    }

    #[test]
    fn a_land_animal_cannot_live_in_the_sea_and_the_reverse() {
        let land = a_species(10.0, 0.0, 30.0, 0.0);
        let mut sea = land.clone();
        sea.marine = true;
        assert_eq!(land.suitability(15.0, 0.0, Biome::Pelagic), 0.0);
        assert_eq!(sea.suitability(15.0, 900.0, Biome::Grassland), 0.0);
        assert!(sea.suitability(15.0, 0.0, Biome::Pelagic) > 0.0);
    }

    #[test]
    fn nothing_lives_on_a_glacier() {
        let s = a_species(10.0, -40.0, 40.0, 0.0);
        assert_eq!(s.suitability(-20.0, 400.0, Biome::Glacier), 0.0);
    }

    #[test]
    fn grass_feeds_grazers_far_better_than_forest_does() {
        // Which is why the herds are on the plains. Nothing decides this; it is asked of
        // the biome.
        assert!(edible_share(Biome::Grassland) > edible_share(Biome::Rainforest) * 4.0);
        assert_eq!(edible_share(Biome::Glacier), 0.0);
        assert!(edible_share(Biome::Pelagic) > 0.5, "plankton is all edible");
    }

    #[test]
    fn random_species_are_varied_and_plausible() {
        let mut rng = WorldSeed::from_u128(0x1).stream(Domain::Ecology, 0, 0);
        let drawn: Vec<Species> = (0..300)
            .map(|i| Species::random(Trophic::Herbivore, i % 4 == 0, 0.0, &mut rng))
            .collect();

        for s in &drawn {
            assert!(s.mass_kg > 0.0 && s.mass_kg < 2000.0);
            assert!(s.tolerance_c() > 0.0);
        }
        // Many small things and few large ones, which is how body sizes really run.
        let small = drawn.iter().filter(|s| s.mass_kg < 10.0).count();
        assert!(small > drawn.len() / 3, "only {small} small species of 300");
        // And a spread of tolerances, so some are specialists and some generalists.
        let narrow = drawn.iter().filter(|s| s.tolerance_c() < 12.0).count();
        let broad = drawn.iter().filter(|s| s.tolerance_c() > 24.0).count();
        assert!(narrow > 20 && broad > 20, "{narrow} narrow, {broad} broad");
    }
}
