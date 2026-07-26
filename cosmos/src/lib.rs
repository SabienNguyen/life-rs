//! Stars, and the worlds that go round them.
//!
//! The project is called a simulation of the universe and until now its universe was one
//! sun, hardcoded, with a brightness curve fitted to it. Everything above this — the
//! energy balance, the carbon thermostat, the ice — already takes a solar constant as an
//! argument, so what was missing was not machinery but a *source*: something to say where
//! that number comes from, and therefore what a world that is not Earth would be like.
//!
//! ## What a star is here
//!
//! One number, and everything else follows from it. A main-sequence star's mass fixes its
//! luminosity, its surface temperature, how long it lives, and how fast it brightens while
//! it does — which is as close to a free lunch as astrophysics offers, and the reason a
//! stellar model this small is worth having at all.
//!
//! The mass–luminosity relation is `L ∝ M^3.5` over the range that matters. It is
//! empirical, it is not exact, and its consequences are enormous: a star half again the
//! sun's mass is four times as bright and burns out in a tenth of the time, so nothing
//! living on its planets gets past the equivalent of the Cambrian. A star at half a solar
//! mass is a thirtieth as bright and lasts fifty gigayears.
//!
//! ## What is not here
//!
//! Binaries, giant branches, metallicity, planetary formation of any kind, orbital
//! dynamics, moons, and tides. Orbits are circular and fixed — eccentricity's effect on
//! the annual-mean insolation is second order, which is the same reason `climate` ignores
//! it — and a system's planets are drawn rather than accreted.

use sim_core::Rng;

pub mod habitability;

/// The sun's luminosity, in watts. Only used to make the ratios read as ratios.
pub const SOLAR_LUMINOSITY_W: f64 = 3.828e26;
/// The solar constant at one astronomical unit, in W/m².
pub const SOLAR_CONSTANT_WM2: f64 = 1361.0;
/// How long the sun spends on the main sequence, in gigayears.
pub const SOLAR_LIFETIME_GYR: f64 = 10.0;
/// The sun's surface temperature, in kelvin.
pub const SOLAR_SURFACE_K: f64 = 5772.0;

/// How much a star brightens across its main-sequence life, as the coefficient in
/// `L ∝ 1/(1 + k(1 − f))` where `f` is the fraction of that life elapsed.
///
/// Calibrated against the one star anybody has measured properly: the sun was about
/// seven tenths of its present output when the Earth formed, and it is a little under
/// half way through. That fixes `k`, and it implies the sun ends its life near twice
/// today's brightness — which is what the stellar-evolution codes say, and is not a
/// number this was fitted to.
const BRIGHTENING: f64 = 1.67;
/// How far through its main-sequence life the sun is now.
///
/// The normalising point for everything else here: "one solar luminosity" means the
/// sun's output *now*, not at birth, so the model has to know which point that is.
const SUN_THROUGH_LIFE: f64 = 0.457;

/// How bright a star is at a given fraction of its life, relative to the sun today.
pub fn brightening_at(through_life: f64) -> f64 {
    let at = |f: f64| 1.0 / (1.0 + BRIGHTENING * (1.0 - f));
    at(through_life.clamp(0.0, 1.0)) / at(SUN_THROUGH_LIFE)
}

/// How old the universe is, in gigayears.
///
/// A cap on how old a drawn star can be, and a real constraint rather than tidiness: a
/// red dwarf's main-sequence life runs to hundreds of gigayears, so drawing uniformly
/// across it produces stars older than everything.
pub const UNIVERSE_GYR: f64 = 13.8;

/// The lightest star that fuses hydrogen, in solar masses.
///
/// Below about eight hundredths of a solar mass a body never gets hot enough in the core
/// and is a brown dwarf instead.
pub const LIGHTEST_STAR: f64 = 0.08;
/// The heaviest star worth drawing for a world with life on it, in solar masses.
///
/// Anything much above this lives for less time than it took the Earth to make oxygen.
pub const HEAVIEST_STAR: f64 = 2.2;

/// A main-sequence star.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Star {
    /// Mass in solar masses. Everything else about it follows from this.
    pub mass_solar: f64,
    /// How far through its life it is, in gigayears.
    pub age_gyr: f64,
}

impl Star {
    pub const SUN: Star = Star {
        mass_solar: 1.0,
        age_gyr: 4.57,
    };

    /// Draw a star.
    ///
    /// Weighted towards the small end, because that is what the sky is: the initial mass
    /// function falls off steeply, and the overwhelming majority of stars are lighter than
    /// the sun. Drawing uniformly in mass would make a galaxy of blue giants, which is
    /// both wrong and much less interesting — a heavy star's planets have no time to get
    /// anywhere.
    pub fn drawn(rng: &mut Rng) -> Star {
        // A power law in mass, sampled by inversion. The Salpeter slope is −2.35; this is
        // gentler, because the very bottom of the range is of no interest to anybody
        // looking for a world and sampling it faithfully would spend nine draws in ten
        // there.
        const SLOPE: f64 = -1.6;
        let u = rng.unit_f64().clamp(1e-9, 1.0 - 1e-9);
        let low = LIGHTEST_STAR.powf(SLOPE + 1.0);
        let high = HEAVIEST_STAR.powf(SLOPE + 1.0);
        let mass = (low + u * (high - low)).powf(1.0 / (SLOPE + 1.0));

        // Somewhere in the first four fifths of its life. Not the last fifth: a star
        // leaving the main sequence is a different problem and this model does not have it.
        let mass_solar = mass.clamp(LIGHTEST_STAR, HEAVIEST_STAR);
        // Somewhere in the first four fifths of its life, and never older than the
        // universe — which binds for every star lighter than the sun, since their lives
        // are longer than everything that has happened.
        let lifetime = main_sequence_gyr(mass_solar);
        let oldest = (0.8 * lifetime).min(UNIVERSE_GYR - 0.2);
        Star {
            mass_solar,
            age_gyr: rng.range_f64(0.15 * lifetime.min(UNIVERSE_GYR), oldest.max(0.05)),
        }
    }

    /// Luminosity now, in solar luminosities.
    ///
    /// Two effects, and they multiply. The mass–luminosity relation gives what a star of
    /// this mass puts out at the sun's stage of life; the brightening curve carries it to
    /// the stage this one is actually at. That second term is what makes the faint young
    /// sun a problem needing a thermostat, and it is not small — the sun is nearly half
    /// again as bright as it was when the Earth formed, and will be twice as bright as
    /// today before it leaves the main sequence.
    pub fn luminosity_solar(&self) -> f64 {
        mass_luminosity(self.mass_solar) * self.brightening()
    }

    /// How bright this star is relative to the sun *today*, from where it is in its life.
    ///
    /// The same shape for every star, measured in units of its own lifetime — which is the
    /// only way to state it that generalises. A heavy star runs through the whole curve in
    /// a few hundred megayears; a red dwarf takes hundreds of gigayears and has barely
    /// started.
    pub fn brightening(&self) -> f64 {
        brightening_at(self.age_gyr / self.main_sequence_gyr())
    }

    /// How far through its main-sequence life this star is, 0 to 1.
    pub fn through_life(&self) -> f64 {
        (self.age_gyr / self.main_sequence_gyr()).clamp(0.0, 1.0)
    }

    /// How long this star spends on the main sequence, in gigayears.
    pub fn main_sequence_gyr(&self) -> f64 {
        main_sequence_gyr(self.mass_solar)
    }

    /// How much of its life is left, in gigayears.
    pub fn remaining_gyr(&self) -> f64 {
        (self.main_sequence_gyr() - self.age_gyr).max(0.0)
    }

    /// Surface temperature, in kelvin.
    ///
    /// From the luminosity and a mass–radius relation, via Stefan–Boltzmann. It decides
    /// the colour of the light, which decides how much of it a snowfield reflects — ice is
    /// far less reflective in the near infrared, so a planet round a red dwarf has a much
    /// weaker ice–albedo feedback than one round the sun. That is a real and load-bearing
    /// difference and this model does not yet use it; the number is here so that it can.
    pub fn surface_k(&self) -> f64 {
        let radius = self.mass_solar.powf(0.8);
        SOLAR_SURFACE_K * (self.luminosity_solar() / (radius * radius)).powf(0.25)
    }

    /// "a" or "an", for whichever colour this star is.
    pub fn article(&self) -> &'static str {
        if self.colour().starts_with('o') { "an" } else { "a" }
    }

    /// What the star looks like, in one word.
    pub fn colour(&self) -> &'static str {
        match self.surface_k() {
            k if k < 3_700.0 => "red",
            k if k < 5_200.0 => "orange",
            k if k < 6_000.0 => "yellow",
            k if k < 7_500.0 => "yellow-white",
            _ => "white",
        }
    }

    /// The flux reaching a circular orbit, in W/m².
    pub fn flux_at_au(&self, au: f64) -> f64 {
        if au <= 0.0 {
            return f64::INFINITY;
        }
        SOLAR_CONSTANT_WM2 * self.luminosity_solar() / (au * au)
    }

    /// The orbit that receives exactly as much light as the Earth does.
    ///
    /// The natural unit for talking about where a world sits, and the number a system
    /// draws its planets around.
    pub fn earthlike_au(&self) -> f64 {
        self.luminosity_solar().sqrt()
    }
}

/// What a star of this mass puts out at the sun's stage of life, in solar luminosities.
///
/// The mass–luminosity relation, in the piecewise form that fits the observations. The
/// exponent is not one number across the whole range — it is steeper for low-mass stars
/// and shallower for high-mass ones — and using a single 3.5 everywhere puts red dwarfs
/// out by a factor of several.
///
/// "At the sun's stage" rather than "at birth" because that is what the relation is
/// fitted to: a sample of main-sequence stars caught at whatever age they happened to be.
pub fn mass_luminosity(mass_solar: f64) -> f64 {
    let m = mass_solar.max(1e-6);
    if m < 0.43 {
        0.23 * m.powf(2.3)
    } else if m < 2.0 {
        m.powf(4.0)
    } else {
        1.4 * m.powf(3.5)
    }
}

/// How long a star of this mass lasts on the main sequence, in gigayears.
///
/// Fuel goes as the mass and burn rate goes as the luminosity, so lifetime goes as
/// `M / L`. The consequence is the single most important fact about stars for anything
/// hoping to live near one: a star twice the sun's mass has twice the fuel and burns it
/// sixteen times as fast.
pub fn main_sequence_gyr(mass_solar: f64) -> f64 {
    let m = mass_solar.max(1e-6);
    SOLAR_LIFETIME_GYR * m / mass_luminosity(m)
}

/// A world going round a star.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbit {
    pub semi_major_au: f64,
    /// Mass in Earth masses.
    pub mass_earth: f64,
}

impl Orbit {
    /// The flux this world receives, in W/m².
    pub fn flux(&self, star: &Star) -> f64 {
        star.flux_at_au(self.semi_major_au)
    }

    /// Year length in Earth years, from Kepler's third law.
    pub fn year_years(&self, star: &Star) -> f64 {
        (self.semi_major_au.powi(3) / star.mass_solar).sqrt()
    }

    /// Surface gravity relative to Earth's, assuming Earth's density.
    ///
    /// `g ∝ M / R²` and `R ∝ M^(1/3)` at fixed density, so `g ∝ M^(1/3)`. Real rocky
    /// planets compress under their own weight and the exponent is nearer 0.5; this is the
    /// simpler version and it is only used for description.
    pub fn gravity(&self) -> f64 {
        self.mass_earth.max(1e-6).powf(1.0 / 3.0)
    }

    /// Whether this world is the sort of thing a surface could exist on.
    ///
    /// Not habitability — that is `habitability` — but the prior question of whether it is
    /// rock at all. Above about ten Earth masses a world holds onto its hydrogen and is a
    /// gas giant, and there is no ground.
    pub fn is_rocky(&self) -> bool {
        self.mass_earth <= 10.0
    }
}

/// A star and its planets.
pub struct System {
    pub star: Star,
    pub worlds: Vec<Orbit>,
}

impl System {
    /// Draw a system.
    ///
    /// Orbits spaced geometrically, which is what the real ones do — the ratio between
    /// successive semi-major axes is roughly constant, an observation old enough to have
    /// been mistaken for a law twice. Masses fall with distance out to the snow line and
    /// jump beyond it, because past the point where water is ice there is far more solid
    /// material to build from. That is why the giants are outside and the rock is inside,
    /// in this system and in most of the real ones.
    pub fn drawn(rng: &mut Rng) -> System {
        let star = Star::drawn(rng);
        let snow_line = 2.7 * star.earthlike_au();

        let count = rng.range_u64(2, 8) as usize;
        let mut worlds = Vec::with_capacity(count);
        let mut au = star.earthlike_au() * rng.range_f64(0.12, 0.55);
        for _ in 0..count {
            let beyond_snow = au > snow_line;
            // Masses drawn log-uniformly, which is both what the surveys find and what
            // makes the arithmetic come out at the measured answer. Drawing them
            // uniformly puts almost every inner planet above the mass that can hold an
            // atmosphere, and four systems in five then come out with a comfortable Earth
            // in them — against a measured `eta-Earth` nearer a third.
            let mass_earth = if beyond_snow {
                log_uniform(rng, 6.0, 340.0)
            } else {
                log_uniform(rng, 0.015, 9.0)
            };
            worlds.push(Orbit {
                semi_major_au: au,
                mass_earth,
            });
            au *= rng.range_f64(1.45, 2.4);
        }
        System { star, worlds }
    }

    /// The world most worth living on, if any is.
    ///
    /// Rocky, in the habitable zone, and of the best size — which is a real filter and
    /// most systems fail it. That is the point: a universe where every star has an Earth
    /// is not a universe, it is a backdrop.
    pub fn best_world(&self) -> Option<usize> {
        let zone = habitability::zone(&self.star);
        let _ = zone;
        self.worlds
            .iter()
            .enumerate()
            .filter(|(_, world)| habitability::promise(&self.star, world) > 0.0)
            .max_by(|(_, a), (_, b)| {
                habitability::promise(&self.star, a).total_cmp(&habitability::promise(&self.star, b))
            })
            .map(|(i, _)| i)
    }
}

/// A draw that is uniform in the logarithm — the right shape for a quantity that spans
/// orders of magnitude, which planetary masses do.
fn log_uniform(rng: &mut Rng, low: f64, high: f64) -> f64 {
    (rng.range_f64(low.ln(), high.ln())).exp()
}

#[cfg(test)]
mod tests;
