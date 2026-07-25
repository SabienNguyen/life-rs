//! The climate on top of the solid planet, and the loop that keeps it habitable.
//!
//! Four things, each with its own module and each answerable on its own: how much
//! sunlight arrives ([`insolation`]), where the heat ends up ([`energy`]), where the
//! rain falls ([`moisture`]), and how much carbon dioxide there is to trap the heat in
//! the first place ([`carbon`]).
//!
//! What makes it a system rather than four calculations is that the last one depends on
//! the other three and they all depend on it. Volcanoes at plate boundaries supply
//! carbon; warmth and rain on continents remove it; carbon sets the temperature;
//! temperature sets the rain. So the planet has a thermostat, and the thermostat is
//! wired to the tectonics — which is the single most important feedback on a rocky
//! planet's surface, and the reason a world can keep liquid water for four billion years
//! while its sun brightens by a third.
//!
//! ## Where the ocean is
//!
//! The roadmap pairs climate with oceans. What is here is the ocean's *climatic* role,
//! which is most of its effect on the surface: it carries heat several times better than
//! land does, which is what makes maritime climates mild and continental interiors
//! extreme; it is the source of essentially all the water in the air; and it freezes.
//! What is not here is circulation as a thing in itself — gyres, overturning, upwelling,
//! nutrients, anoxia. Those belong with the ecology that consumes them, and building
//! them before there is anything to eat the plankton would be building a number nobody
//! reads.

pub mod carbon;
pub mod energy;
pub mod insolation;
pub mod moisture;
pub mod sphere;

use geo::{CellId, Lithosphere};
use sim_core::Rng;

pub use energy::{Energy, ICE_POINT};
pub use moisture::Moisture;

/// How many rounds the energy balance relaxes for on an ordinary step.
///
/// The climate is being restarted from where it already was rather than from scratch, so
/// after the first settling it only has to follow whatever moved.
const RELAX_ROUNDS: usize = 60;
const SETTLE_ROUNDS: usize = 400;
/// How many alternating temperature-and-carbon passes a cold start takes.
const SETTLE_PASSES: usize = 60;
const MOISTURE_ROUNDS: usize = 40;
/// How many times temperature, rain and carbon are solved against each other per step.
///
/// They are mutually dependent and the dependence is gentle, so a few passes is enough;
/// what it must not be is one, because then a step's carbon is answering last step's
/// temperature and a warming planet chases its own tail.
const COUPLING_PASSES: usize = 3;

/// Everything about the surface that is not rock.
pub struct Climate {
    energy: Energy,
    moisture: Moisture,
    insolation: Vec<f32>,
    surface_c: Vec<f32>,
    co2_ppm: f32,
    obliquity_deg: f64,
    /// Age of the system in gigayears, which is what sets how bright the sun is.
    age_gyr: f64,
    settled: bool,
}

impl Climate {
    /// A climate for a planet, solved from cold — which is to say from a uniform
    /// fourteen degrees, and then relaxed until it stops moving.
    pub fn genesis(planet: &Lithosphere, age_gyr: f64, obliquity_deg: f64) -> Climate {
        let grid = planet.grid();
        let mut climate = Climate {
            energy: Energy::new(grid),
            moisture: Moisture::new(grid.len()),
            insolation: vec![0.0; grid.len()],
            surface_c: vec![14.0; grid.len()],
            co2_ppm: carbon::REFERENCE_CO2_PPM,
            obliquity_deg,
            age_gyr,
            settled: false,
        };
        climate.settle(planet);
        climate
    }

    // ---- reading it ------------------------------------------------------------

    pub fn temperature_c(&self, cell: CellId) -> f32 {
        self.energy.surface_c(cell)
    }

    pub fn rain_mm(&self, cell: CellId) -> f32 {
        self.moisture.rain_mm(cell)
    }

    pub fn is_frozen(&self, cell: CellId) -> bool {
        self.energy.is_frozen(cell)
    }

    pub fn co2_ppm(&self) -> f32 {
        self.co2_ppm
    }

    pub fn obliquity_deg(&self) -> f64 {
        self.obliquity_deg
    }

    pub fn age_gyr(&self) -> f64 {
        self.age_gyr
    }

    /// How bright the sun is now, relative to today's.
    pub fn brightness(&self) -> f64 {
        insolation::brightness_at(self.age_gyr)
    }

    pub fn mean_temperature_c(&self, planet: &Lithosphere) -> f32 {
        self.energy.mean_c(planet)
    }

    pub fn ice_fraction(&self, planet: &Lithosphere) -> f32 {
        self.energy.ice_fraction(planet)
    }

    /// Mean rainfall over the planet, in millimetres a year.
    pub fn mean_rain_mm(&self, planet: &Lithosphere) -> f32 {
        weighted(planet, |c| self.moisture.rain_mm(c))
    }

    /// Mean temperature of the dry land, in °C — the thing weathering responds to.
    pub fn land_temperature_c(&self, planet: &Lithosphere) -> f32 {
        over_land(planet, |c| self.energy.surface_c(c)).unwrap_or(0.0)
    }

    /// Mean rainfall on dry land, relative to the reference planet.
    pub fn land_runoff(&self, planet: &Lithosphere) -> f32 {
        over_land(planet, |c| self.moisture.runoff(c)).unwrap_or(0.0)
    }

    /// Share of the surface that could hold liquid water at all: not frozen, not
    /// boiling. A crude habitability index, and the one the deep-time view wants.
    pub fn temperate_fraction(&self, planet: &Lithosphere) -> f32 {
        let grid = planet.grid();
        let mut good = 0.0;
        let mut all = 0.0;
        for cell in grid.cells() {
            let area = grid.solid_angle(cell);
            all += area;
            let t = self.energy.surface_c(cell);
            if (0.0..60.0).contains(&t) {
                good += area;
            }
        }
        (good / all) as f32
    }

    // ---- running it ------------------------------------------------------------

    /// Bring the whole system to steady state from wherever it is.
    ///
    /// Used at genesis and after anything that moves a continent a long way. Costs a few
    /// hundred relaxation rounds instead of a few dozen.
    pub fn settle(&mut self, planet: &Lithosphere) {
        self.recompute_insolation(planet);
        // Many short passes rather than a few long ones, and that is not a tuning
        // choice. Solving the temperature all the way down before letting the carbon
        // answer means a planet under a faint sun freezes solid first and *then* starts
        // accumulating carbon dioxide — by which point its albedo has doubled and no
        // achievable amount of carbon can lift it out again. The real young Earth was
        // never briefly frozen with modern carbon dioxide; it had a thick atmosphere
        // precisely because weathering had been slow. Letting the two move together is
        // what reproduces that.
        for _ in 0..SETTLE_PASSES {
            self.solve(planet, RELAX_ROUNDS / 2);
            self.balance_carbon(planet, 2.0);
        }
        self.solve(planet, SETTLE_ROUNDS);
        self.settled = true;
    }

    /// Advance the climate by a span, following whatever the planet has done.
    ///
    /// The climate itself has no memory worth a megayear — it equilibrates in centuries.
    /// What has memory is the carbon, and it is the carbon that this actually steps; the
    /// temperature and rainfall are recomputed each time from where the continents now
    /// are and how much carbon is in the air.
    pub fn step_myr(&mut self, planet: &Lithosphere, dt: f32, _rng: &mut Rng) {
        debug_assert!(dt > 0.0, "time only runs forwards");
        self.age_gyr += dt as f64 / 1000.0;
        self.recompute_insolation(planet);
        for _ in 0..COUPLING_PASSES {
            self.solve(planet, RELAX_ROUNDS);
            self.balance_carbon(planet, dt / COUPLING_PASSES as f32);
        }
        self.solve(planet, RELAX_ROUNDS);
    }

    fn recompute_insolation(&mut self, planet: &Lithosphere) {
        let grid = planet.grid();
        let sun = insolation::SOLAR_CONSTANT * self.brightness();
        for cell in grid.cells() {
            let latitude = grid.position(cell).latitude();
            self.insolation[cell as usize] =
                insolation::annual_mean(latitude, self.obliquity_deg, sun) as f32;
        }
    }

    fn solve(&mut self, planet: &Lithosphere, rounds: usize) {
        self.energy
            .solve(planet, &self.insolation, self.co2_ppm, rounds);
        for cell in planet.grid().cells() {
            self.surface_c[cell as usize] = self.energy.surface_c(cell);
        }
        self.moisture
            .settle(planet, &self.surface_c, MOISTURE_ROUNDS);
    }

    fn balance_carbon(&mut self, planet: &Lithosphere, dt: f32) {
        let supply = carbon::outgassing(planet);
        let land = planet.land_fraction();
        let demand = carbon::weathering(
            self.co2_ppm,
            self.land_temperature_c(planet),
            land,
            self.land_runoff(planet),
        );
        self.co2_ppm = carbon::relax(self.co2_ppm, supply, demand, dt);
    }
}

fn weighted(planet: &Lithosphere, of: impl Fn(CellId) -> f32) -> f32 {
    let grid = planet.grid();
    let mut total = 0.0;
    let mut area = 0.0;
    for cell in grid.cells() {
        let a = grid.solid_angle(cell);
        total += of(cell) as f64 * a;
        area += a;
    }
    (total / area) as f32
}

fn over_land(planet: &Lithosphere, of: impl Fn(CellId) -> f32) -> Option<f32> {
    let grid = planet.grid();
    let mut total = 0.0;
    let mut area = 0.0;
    for cell in grid.cells() {
        if !planet.is_land(cell) {
            continue;
        }
        let a = grid.solid_angle(cell);
        total += of(cell) as f64 * a;
        area += a;
    }
    (area > 0.0).then(|| (total / area) as f32)
}

#[cfg(test)]
mod tests;
