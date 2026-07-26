//! Where a world can have liquid water, and for how long.
//!
//! The habitable zone is the most over-quoted idea in the subject and it is worth being
//! precise about what it actually is: the band of orbits where an Earth-like planet with a
//! carbonate–silicate thermostat can hold liquid water *somewhere* on its surface. It is
//! not a promise of an ocean, or of life, or of anything except that the arithmetic does
//! not immediately rule it out.
//!
//! It is also wider than the naive answer, and for a reason this project already models.
//! Move a planet outwards and it cools, so rain washes less carbon dioxide out of the air,
//! so the greenhouse strengthens until it is warm again. The outer edge is where that runs
//! out — where the carbon dioxide is thick enough to start condensing and scattering light
//! back to space rather than trapping it. The inner edge is the other runaway: warm enough
//! that water vapour, itself a greenhouse gas, feeds its own evaporation.
//!
//! The bounds here are the Kopparapu ones, which are the standard modern calculation, in
//! the conservative form — runaway greenhouse to maximum greenhouse rather than the
//! optimistic recent-Venus-to-early-Mars limits.

use crate::{Orbit, Star};

/// The flux at the inner edge, relative to Earth's, for a sun-like star.
///
/// Above this, water vapour feeds its own evaporation and the ocean leaves.
const RUNAWAY_GREENHOUSE: f64 = 1.107;
/// The flux at the outer edge, relative to Earth's.
///
/// Below this, no achievable amount of carbon dioxide keeps the surface above freezing —
/// past a point the gas scatters more light away than it traps.
const MAXIMUM_GREENHOUSE: f64 = 0.356;

/// A band of orbits, in astronomical units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub inner_au: f64,
    pub outer_au: f64,
}

impl Zone {
    pub fn holds(&self, au: f64) -> bool {
        (self.inner_au..=self.outer_au).contains(&au)
    }

    /// How wide the band is, as a fraction of its own middle.
    ///
    /// The number that says how lucky a planet has to be. It is the same for every star
    /// of the same colour, which is the useful part: the zone moves with luminosity but
    /// its *shape* does not.
    pub fn generosity(&self) -> f64 {
        let middle = (self.inner_au + self.outer_au) / 2.0;
        if middle <= 0.0 {
            0.0
        } else {
            (self.outer_au - self.inner_au) / middle
        }
    }
}

/// The habitable zone of a star, now.
///
/// "Now" matters and is usually left out. The zone moves outwards as a star brightens, so
/// a world comfortable at one gigayear can be uninhabitable at five without having moved
/// at all. That is the fate of the Earth, and it is a billion years away rather than five.
pub fn zone(star: &Star) -> Zone {
    let luminosity = star.luminosity_solar();
    Zone {
        inner_au: (luminosity / RUNAWAY_GREENHOUSE).sqrt(),
        outer_au: (luminosity / MAXIMUM_GREENHOUSE).sqrt(),
    }
}

/// The band a world stays inside for the whole of its star's main-sequence life.
///
/// Much narrower than the instantaneous zone, and much more interesting: it is where a
/// biosphere can get somewhere. A planet that spends a gigayear habitable and then boils
/// has time for single cells and nothing else.
pub fn continuous_zone(star: &Star) -> Zone {
    let young = Star {
        age_gyr: 0.0,
        ..*star
    };
    let old = Star {
        age_gyr: star.main_sequence_gyr(),
        ..*star
    };
    // Inside is set by the end of life, when the star is brightest; outside by the
    // beginning, when it is faintest. If they cross, there is no such band.
    Zone {
        inner_au: zone(&old).inner_au,
        outer_au: zone(&young).outer_au,
    }
}

/// How promising a world is, 0 to 1 — worth looking at, rather than worth living on.
///
/// Three things, multiplied, for the reason `settlement` multiplies: a world has to be the
/// right size, and the right distance, and have time, and failing any one of them is
/// failing. A sum would rank a scorching super-Earth over a temperate one.
pub fn promise(star: &Star, world: &Orbit) -> f64 {
    if !world.is_rocky() {
        return 0.0;
    }
    let zone = zone(star);
    if !zone.holds(world.semi_major_au) {
        return 0.0;
    }

    // Where in the band it sits. Best in the middle, which is where a thermostat has the
    // most room to move in both directions before it runs out of answers.
    let across = (world.semi_major_au - zone.inner_au) / (zone.outer_au - zone.inner_au);
    let placed = 1.0 - (2.0 * across - 1.0).abs().powi(2);

    placed * body_and_time(star, world)
}

/// Everything about a world's promise except where it sits — its size, and how long its
/// star has left.
///
/// Split out because *where it sits* is the one term a caller may reasonably disagree
/// about. This crate says the middle of the astronomical habitable zone is best, which is
/// correct as astronomy; a caller with an actual climate model may find its own band is
/// somewhere else entirely, and should be able to substitute its own placement without
/// losing the rest. `sim` does exactly that.
pub fn body_and_time(star: &Star, world: &Orbit) -> f64 {
    if !world.is_rocky() {
        return 0.0;
    }
    // Size. Too small and the atmosphere leaves and the interior freezes, which stops the
    // plate tectonics the carbon cycle runs on; too large and it holds its hydrogen and
    // has no surface to speak of. Earth is not special here so much as *sufficient*, and
    // the useful range is wide — but the bottom of it is a hard edge rather than a taper.
    // Mars is a tenth of an Earth mass, sits at the outer edge of the sun's habitable
    // zone, and lost its air anyway.
    let sized = match world.mass_earth {
        m if m < 0.3 => 0.0,
        m if m < 1.0 => ((m - 0.3) / 0.7).powf(0.6),
        m if m <= 5.0 => 1.0 - 0.08 * (m - 1.0),
        m => (1.0 - 0.16 * (m - 5.0)).max(0.0),
    };

    // Tidal locking, which is the single largest problem with the places most stars keep
    // their habitable zones. A dim star's zone is close in, and close in the tide raised
    // by the star despins the planet until one face is permanent day. Whether that is
    // fatal is genuinely open — a thick enough atmosphere carries heat to the night side —
    // so this is a heavy penalty rather than a veto. It matters enormously here because
    // most stars are red dwarfs, so without it four systems in five come out with a
    // comfortable Earth in them.
    let free = if locked(star, world) { 0.25 } else { 1.0 };

    // Time. Not how long the star lives but how long it has *left*, because a biosphere
    // needs a future and not a past. Scaled against the four gigayears the Earth took to
    // get from its first cell to anything that could ask the question.
    let time = (star.remaining_gyr() / 4.0).clamp(0.0, 1.0);

    (sized * time * free).clamp(0.0, 1.0)
}

/// Whether a star has despun a world onto one face.
///
/// The locking radius grows as the cube root of the star's mass — the tide goes as mass
/// over distance cubed and the timescale integrates to something near `a^6 / M^2`, which
/// at a few gigayears comes out here. Mercury is inside the sun's; the Earth is not.
pub fn locked(star: &Star, world: &Orbit) -> bool {
    world.semi_major_au < 0.5 * star.mass_solar.powf(1.0 / 3.0)
}

/// A sentence about a world, for the observer.
pub fn describe(star: &Star, world: &Orbit) -> String {
    let flux = world.flux(star) / crate::SOLAR_CONSTANT_WM2;
    let kind = if !world.is_rocky() {
        "a gas giant"
    } else if flux > RUNAWAY_GREENHOUSE {
        "too close, and boiling"
    } else if flux < MAXIMUM_GREENHOUSE {
        "too far, and frozen"
    } else {
        "in the habitable zone"
    };
    format!(
        "{:.2} AU from {} {} star, {:.0}% of Earth's sunlight, {:.1} Earth masses — {kind}",
        world.semi_major_au,
        star.article(),
        star.colour(),
        flux * 100.0,
        world.mass_earth,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_earth_is_in_the_suns_habitable_zone() {
        let band = zone(&Star::SUN);
        assert!(band.holds(1.0), "{band:?}");
        // And the edges are where the literature puts them, near enough: the conservative
        // inner edge is about 0.95 AU and the outer about 1.67.
        assert!(
            (0.90..1.00).contains(&band.inner_au),
            "inner edge at {:.3} AU",
            band.inner_au
        );
        assert!(
            (1.60..1.75).contains(&band.outer_au),
            "outer edge at {:.3} AU",
            band.outer_au
        );
    }

    #[test]
    fn venus_is_outside_the_zone_and_mars_is_inside_it_and_dead_anyway() {
        let band = zone(&Star::SUN);
        assert!(!band.holds(0.72), "Venus came out habitable");
        // Mars *is* inside the conservative zone — the outer edge is about 1.67 AU and
        // Mars is at 1.52. That is not a bug in the bound, it is the point of the bound:
        // being in the zone is necessary and nowhere near sufficient. What killed Mars is
        // that it is a tenth of an Earth mass and could not hold its air.
        assert!(band.holds(1.52), "Mars came out too far");
        let mars = Orbit { semi_major_au: 1.52, mass_earth: 0.107 };
        assert_eq!(promise(&Star::SUN, &mars), 0.0, "Mars came out promising");
    }

    #[test]
    fn a_red_dwarfs_habitable_zone_is_inside_its_tidal_lock() {
        // The reason most of the sky is a bad address. A dim star keeps its warm orbits
        // close in, and close in the star despins the planet.
        let dwarf = Star { mass_solar: 0.25, age_gyr: 3.0 };
        let band = zone(&dwarf);
        let middle = Orbit {
            semi_major_au: (band.inner_au + band.outer_au) / 2.0,
            mass_earth: 1.0,
        };
        assert!(locked(&dwarf, &middle), "a red dwarf's zone came out free-spinning");

        // The sun does not do this to the Earth, and does to Mercury.
        let earth = Orbit { semi_major_au: 1.0, mass_earth: 1.0 };
        let mercury = Orbit { semi_major_au: 0.39, mass_earth: 0.055 };
        assert!(!locked(&Star::SUN, &earth));
        assert!(locked(&Star::SUN, &mercury));
    }

    #[test]
    fn a_dim_star_keeps_its_worlds_close() {
        let dwarf = Star {
            mass_solar: 0.2,
            age_gyr: 1.0,
        };
        let band = zone(&dwarf);
        assert!(
            band.outer_au < 0.3,
            "a red dwarf's habitable zone reached {:.2} AU",
            band.outer_au
        );
        // And the band is the same *shape* — the zone scales with the square root of
        // luminosity, so its relative width does not depend on the star.
        assert!((band.generosity() - zone(&Star::SUN).generosity()).abs() < 1e-6);
    }

    #[test]
    fn the_zone_moves_outwards_as_a_star_ages() {
        let young = Star {
            mass_solar: 1.0,
            age_gyr: 0.5,
        };
        let old = Star {
            mass_solar: 1.0,
            age_gyr: 9.0,
        };
        assert!(zone(&old).inner_au > zone(&young).inner_au);
        assert!(zone(&old).outer_au > zone(&young).outer_au);
    }

    #[test]
    fn staying_habitable_is_harder_than_being_habitable() {
        let sun = Star::SUN;
        let now = zone(&sun);
        let always = continuous_zone(&sun);
        assert!(always.inner_au > now.inner_au);
        assert!(always.outer_au < now.outer_au);
        assert!(
            always.generosity() < now.generosity(),
            "the continuous band should be the narrower of the two"
        );
        // The Earth is not in it, which is correct and is the interesting part: it will
        // not be habitable for the sun's whole main-sequence life.
        assert!(!always.holds(1.0));
    }

    #[test]
    fn a_gas_giant_in_the_zone_is_still_no_use() {
        let jupiter_where_earth_is = Orbit {
            semi_major_au: 1.0,
            mass_earth: 318.0,
        };
        assert_eq!(promise(&Star::SUN, &jupiter_where_earth_is), 0.0);
    }

    #[test]
    fn the_middle_of_the_band_beats_its_edges() {
        let band = zone(&Star::SUN);
        let at = |au| {
            promise(
                &Star::SUN,
                &Orbit {
                    semi_major_au: au,
                    mass_earth: 1.0,
                },
            )
        };
        let middle = (band.inner_au + band.outer_au) / 2.0;
        assert!(at(middle) > at(band.inner_au + 0.01));
        assert!(at(middle) > at(band.outer_au - 0.01));
    }

    #[test]
    fn a_heavy_star_is_a_bad_bet_however_well_placed_the_planet() {
        // Two gigayears of main sequence. A world in the perfect orbit round it still has
        // no time, and time is a factor rather than a bonus.
        let heavy = Star {
            mass_solar: 2.0,
            age_gyr: 1.0,
        };
        let band = zone(&heavy);
        let world = Orbit {
            semi_major_au: (band.inner_au + band.outer_au) / 2.0,
            mass_earth: 1.0,
        };
        let earth = Orbit {
            semi_major_au: 1.0,
            mass_earth: 1.0,
        };
        assert!(
            promise(&heavy, &world) < promise(&Star::SUN, &earth),
            "a two-solar-mass star scored {:.3} against the sun's {:.3}",
            promise(&heavy, &world),
            promise(&Star::SUN, &earth)
        );
    }

    #[test]
    fn a_world_is_described_by_where_it_is() {
        let sun = Star::SUN;
        let hot = Orbit { semi_major_au: 0.4, mass_earth: 1.0 };
        let cold = Orbit { semi_major_au: 6.0, mass_earth: 1.0 };
        let home = Orbit { semi_major_au: 1.0, mass_earth: 1.0 };
        assert!(describe(&sun, &hot).contains("boiling"));
        assert!(describe(&sun, &cold).contains("frozen"));
        assert!(describe(&sun, &home).contains("habitable zone"));
        assert!(describe(&sun, &home).contains("yellow"));
    }
}
