//! Temperature, from the only thing that ultimately sets it: where the energy goes.
//!
//! Sunlight comes in and is partly reflected; heat leaves as infrared at a rate that
//! rises with temperature and falls with carbon dioxide; and the difference is carried
//! sideways by the atmosphere and ocean from where there is a surplus to where there is
//! a deficit. Three terms. That is an energy-balance model, and it is what climate
//! science ran on for the two decades before general circulation models, because it
//! captures the things that matter over deep time — the mean, the pole-to-equator
//! gradient, the ice line, and the feedbacks that can run away.
//!
//! What it does not capture is weather: no storms, no jet stream, no eddies, no
//! seasons. It answers "what is the climate here" and not "what is it doing today".
//!
//! ## The linear infrared law
//!
//! `OLR = A + B·T` looks too simple to be right, and it is one of the better-founded
//! parts of the model: measured from satellites, outgoing longwave really is close to
//! linear in surface temperature over the range a habitable planet occupies, because
//! the water-vapour feedback that would make it steeply nonlinear is already folded into
//! the fitted slope. The Budyko values are used here: 203.3 W/m² and 2.09 W/m²/K.

use geo::{CellId, Grid, Lithosphere};

/// Outgoing longwave at freezing, and its slope with temperature.
const OLR_BASE: f32 = 203.3;
const OLR_SLOPE: f32 = 2.09;

/// How much the outgoing longwave falls per doubling of carbon dioxide, in W/m².
///
/// The measured radiative forcing of doubled CO₂, and one of the most-checked numbers
/// in the field. Everything about the thermostat rests on it being logarithmic rather
/// than linear: the greenhouse effect saturates, so holding a planet warm as the sun
/// dims takes exponentially more carbon each time.
const CO2_DOUBLING: f32 = 3.7;
/// Carbon dioxide the base infrared law was fitted at, in parts per million.
pub const REFERENCE_CO2_PPM: f32 = 300.0;

/// Planetary albedo, including cloud, for each kind of surface.
const ALBEDO_OCEAN: f32 = 0.29;
const ALBEDO_LAND: f32 = 0.34;
const ALBEDO_ICE: f32 = 0.62;

/// The range over which a surface goes from bare to fully iced, in °C.
///
/// A range rather than a threshold, and it matters. A step function says that a region
/// one degree below freezing reflects twice what a region one degree above it does,
/// which makes the ice–albedo feedback far more violent than it is and opens a
/// hysteresis loop so wide that a frozen planet cannot escape under any amount of
/// carbon dioxide. Real snow cover comes and goes gradually with latitude, altitude and
/// season, and the standard treatment in these models is to smooth the transition.
pub const ICE_POINT: f32 = -8.0;
const ICE_BEGINS: f32 = -1.0;
const ICE_COMPLETE: f32 = -16.0;

/// How readily heat moves polewards, in W/m²/K per radian squared.
///
/// The one genuinely fitted parameter here, and it is fitted to the observable it
/// exists to reproduce: the pole-to-equator temperature difference. Too little and the
/// tropics boil while the poles freeze solid; too much and the planet is isothermal and
/// has no climate zones at all. Around a half is the value the literature settles on.
/// Ocean carries heat considerably better than land, which is why continental interiors
/// are extreme and maritime climates mild — so it depends on what is at both ends.
///
/// Note the units. This is a diffusivity against the *Laplacian*, so turning it into a
/// conductance between two neighbouring cells means dividing by the square of how far
/// apart they are — which is what makes the answer the same whatever the grid level.
/// Leaving that step out is a factor of a couple of hundred at level four, and the
/// planet it produced had a hundred degrees between its equator and its poles.
const DIFFUSIVITY_LAND: f32 = 0.30;
const DIFFUSIVITY_OCEAN: f32 = 0.80;

/// How much colder it gets with height, in °C per kilometre.
///
/// The moist adiabatic lapse rate. Not part of the energy balance — the balance is
/// solved at sea level, and this is applied afterwards, because a mountain is cold for
/// reasons of altitude rather than of radiation budget.
pub const LAPSE_RATE: f32 = 5.5;

/// The energy balance over a grid, solved to steady state.
pub struct Energy {
    /// Cell spacing in radians, squared — the conversion from diffusivity to
    /// conductance between neighbours.
    spacing_sq: f32,
    /// Sea-level temperature in °C, before the lapse rate is applied.
    baseline_c: Vec<f32>,
    /// Surface temperature in °C, with altitude.
    surface_c: Vec<f32>,
    albedo: Vec<f32>,
    absorbed: Vec<f32>,
    scratch: Vec<f32>,
    /// Conductance for each cell against each of its neighbours, in the grid's order.
    conductance: Vec<f32>,
}

impl Energy {
    pub fn new(grid: &Grid) -> Energy {
        let cells = grid.len();
        let edges = cells * geo::grid::MAX_NEIGHBOURS;
        let spacing = grid.spacing_km(geo::EARTH_RADIUS_KM) / geo::EARTH_RADIUS_KM;
        Energy {
            spacing_sq: (spacing * spacing) as f32,
            baseline_c: vec![14.0; cells],
            surface_c: vec![14.0; cells],
            albedo: vec![ALBEDO_OCEAN; cells],
            absorbed: vec![0.0; cells],
            scratch: vec![0.0; cells],
            conductance: vec![0.0; edges],
        }
    }

    pub fn surface_c(&self, cell: CellId) -> f32 {
        self.surface_c[cell as usize]
    }

    pub fn sea_level_c(&self, cell: CellId) -> f32 {
        self.baseline_c[cell as usize]
    }

    pub fn albedo(&self, cell: CellId) -> f32 {
        self.albedo[cell as usize]
    }

    /// Whether this cell carries year-round ice, meaning more of it than not.
    pub fn is_frozen(&self, cell: CellId) -> bool {
        self.surface_c[cell as usize] < ICE_POINT
    }

    /// Solve for the steady state under a given insolation field and carbon dioxide.
    ///
    /// Relaxation rather than a linear solve, because the albedo depends on the
    /// temperature it is trying to find. That feedback is the whole point — it is what
    /// makes an ice age possible and a snowball reachable — and it is also why the
    /// answer is not unique: the same planet under the same sun has both a warm and a
    /// frozen solution, and which one it settles into depends on where it starts. So
    /// this deliberately starts from wherever the climate already was.
    pub fn solve(&mut self, planet: &Lithosphere, insolation: &[f32], co2_ppm: f32, rounds: usize) {
        let grid = planet.grid();
        let base = OLR_BASE - CO2_DOUBLING * (co2_ppm / REFERENCE_CO2_PPM).max(1e-6).log2();

        self.wire(grid, planet);
        for _ in 0..rounds {
            for (cell, sun) in insolation.iter().enumerate().take(grid.len()) {
                let id = cell as CellId;
                let bare = if planet.is_land(id) {
                    ALBEDO_LAND
                } else {
                    ALBEDO_OCEAN
                };
                let iced = ((ICE_BEGINS - self.surface_c[cell]) / (ICE_BEGINS - ICE_COMPLETE))
                    .clamp(0.0, 1.0);
                self.albedo[cell] = bare + (ALBEDO_ICE - bare) * iced;
                self.absorbed[cell] = sun * (1.0 - self.albedo[cell]);
            }

            // One step of the relaxation. Each cell moves towards the temperature that
            // would balance its own budget, given its neighbours as they stand.
            for cell in 0..grid.len() {
                let id = cell as CellId;
                let mut exchange = 0.0;
                let mut leak = 0.0;
                for (slot, &n) in grid.neighbours(id).iter().enumerate() {
                    let k = self.conductance[edge_index(grid, id, slot)];
                    exchange += k * self.baseline_c[n as usize];
                    leak += k;
                }
                // Solving `absorbed = A + B·T + Σk(T − Tₙ)` for T directly, rather than
                // stepping towards it, which converges in a few dozen rounds instead of
                // a few thousand.
                self.scratch[cell] = (self.absorbed[cell] - base + exchange) / (OLR_SLOPE + leak);
            }
            self.baseline_c.copy_from_slice(&self.scratch);

            for cell in 0..grid.len() {
                let height = planet.height_above_sea_m(cell as CellId).max(0.0);
                self.surface_c[cell] = self.baseline_c[cell] - LAPSE_RATE * height / 1000.0;
            }
        }
    }

    /// Set the conductance of every edge from what lies at its two ends.
    ///
    /// On a hexagonal mesh the sum over neighbours of the temperature difference is
    /// three halves of the spacing squared times the Laplacian, so that is the factor
    /// that turns a diffusivity into a conductance.
    fn wire(&mut self, grid: &Grid, planet: &Lithosphere) {
        let scale = 2.0 / (3.0 * self.spacing_sq);
        for cell in grid.cells() {
            for (slot, &n) in grid.neighbours(cell).iter().enumerate() {
                let both_sea = !planet.is_land(cell) && !planet.is_land(n);
                let d = if both_sea {
                    DIFFUSIVITY_OCEAN
                } else {
                    DIFFUSIVITY_LAND
                };
                self.conductance[edge_index(grid, cell, slot)] = d * scale;
            }
        }
    }

    /// Area-weighted mean surface temperature, in °C.
    pub fn mean_c(&self, planet: &Lithosphere) -> f32 {
        let grid = planet.grid();
        let mut total = 0.0;
        let mut area = 0.0;
        for cell in grid.cells() {
            let a = grid.solid_angle(cell);
            total += self.surface_c[cell as usize] as f64 * a;
            area += a;
        }
        (total / area) as f32
    }

    /// Share of the surface under year-round ice.
    pub fn ice_fraction(&self, planet: &Lithosphere) -> f32 {
        let grid = planet.grid();
        let mut frozen = 0.0;
        let mut area = 0.0;
        for cell in grid.cells() {
            let a = grid.solid_angle(cell);
            area += a;
            if self.is_frozen(cell) {
                frozen += a;
            }
        }
        (frozen / area) as f32
    }
}

/// Where a cell's nth neighbour's conductance is stored.
fn edge_index(_grid: &Grid, cell: CellId, slot: usize) -> usize {
    cell as usize * geo::grid::MAX_NEIGHBOURS + slot
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insolation::{EARTH_OBLIQUITY, SOLAR_CONSTANT, annual_mean};
    use sim_core::{Domain, Rng, WorldSeed};

    fn a_planet(seed: u128, land: f32) -> Lithosphere {
        let mut rng: Rng = WorldSeed::from_u128(seed).stream(Domain::Terrain, 0, 0);
        let mut planet = Lithosphere::genesis(4, 9, land, &mut rng);
        // Long enough for plates to have met and thrown up some relief; a planet at
        // genesis is flat, and half of what these tests ask about is altitude.
        for _ in 0..40 {
            planet.step_myr(4.0, &mut rng);
        }
        planet
    }

    fn sunlight(planet: &Lithosphere, brightness: f64) -> Vec<f32> {
        planet
            .grid()
            .cells()
            .map(|c| {
                let lat = planet.grid().position(c).latitude();
                annual_mean(lat, EARTH_OBLIQUITY, SOLAR_CONSTANT * brightness) as f32
            })
            .collect()
    }

    fn settled(planet: &Lithosphere, brightness: f64, co2: f32) -> Energy {
        let mut energy = Energy::new(planet.grid());
        energy.solve(planet, &sunlight(planet, brightness), co2, 300);
        energy
    }

    #[test]
    fn an_earthlike_planet_comes_out_at_an_earthlike_temperature() {
        let planet = a_planet(0x1, 0.42);
        let energy = settled(&planet, 1.0, REFERENCE_CO2_PPM);
        let mean = energy.mean_c(&planet);
        assert!(
            (5.0..22.0).contains(&mean),
            "global mean temperature was {mean:.1} °C"
        );
    }

    #[test]
    fn it_is_hot_at_the_equator_and_cold_at_the_poles() {
        // And by roughly the right amount: Earth runs about 27 °C at the equator and
        // −25 at the poles, a gradient of some fifty degrees. That difference is what
        // the transport coefficient is fitted to, so this is the check on the fit.
        let planet = a_planet(0x2, 0.42);
        let energy = settled(&planet, 1.0, REFERENCE_CO2_PPM);

        let band = |low: f64, high: f64| {
            let cells: Vec<CellId> = planet
                .grid()
                .cells()
                .filter(|c| {
                    let lat = planet.grid().position(*c).latitude().to_degrees().abs();
                    (low..high).contains(&lat)
                })
                .collect();
            cells.iter().map(|c| energy.sea_level_c(*c)).sum::<f32>() / cells.len() as f32
        };

        let tropics = band(0.0, 15.0);
        let poles = band(75.0, 91.0);
        assert!(
            (20.0..34.0).contains(&tropics),
            "the tropics came out at {tropics:.1} °C"
        );
        assert!(
            (-40.0..-8.0).contains(&poles),
            "the poles came out at {poles:.1} °C"
        );
        assert!(
            (35.0..70.0).contains(&(tropics - poles)),
            "the pole-to-equator gradient was {:.1} °C",
            tropics - poles
        );
    }

    #[test]
    fn the_ice_line_sits_in_high_latitudes() {
        // At sea level. Ice at low latitude is a separate matter and a real one — see
        // the next test — so this asks the question the ice line is actually about.
        let planet = a_planet(0x3, 0.42);
        let energy = settled(&planet, 1.0, REFERENCE_CO2_PPM);
        let mut lowest = 91.0f64;
        for cell in planet.grid().cells() {
            if energy.sea_level_c(cell) < ICE_POINT {
                lowest = lowest.min(planet.grid().position(cell).latitude().to_degrees().abs());
            }
        }
        assert!(
            lowest < 85.0,
            "no ice at all; the lowest frozen latitude was {lowest}"
        );
        assert!(
            lowest > 45.0,
            "sea-level ice reached {lowest:.0}° of latitude on a temperate planet"
        );
    }

    #[test]
    fn there_is_ice_in_the_tropics_but_only_on_the_high_ground() {
        // Kilimanjaro and the Andes: three degrees from the equator and under permanent
        // snow, because five kilometres up is twenty-seven degrees colder. It falls out
        // of the lapse rate rather than being a special case.
        let planet = a_planet(0x3, 0.42);
        let energy = settled(&planet, 1.0, REFERENCE_CO2_PPM);
        for cell in planet.grid().cells() {
            let lat = planet.grid().position(cell).latitude().to_degrees().abs();
            if lat < 35.0 && energy.is_frozen(cell) {
                assert!(
                    planet.height_above_sea_m(cell) > 2500.0,
                    "cell {cell} at {lat:.0}° is frozen at only {:.0} m",
                    planet.height_above_sea_m(cell)
                );
            }
        }
    }

    #[test]
    fn mountains_are_cold() {
        // Not because of the radiation budget — the balance is solved at sea level —
        // but because the air above them is thinner. Two cells at the same latitude
        // should differ by the lapse rate and nothing else.
        let planet = a_planet(0x4, 0.42);
        let energy = settled(&planet, 1.0, REFERENCE_CO2_PPM);
        let mut checked = 0;
        for cell in planet.grid().cells() {
            let height = planet.height_above_sea_m(cell);
            if height > 2000.0 {
                let drop = energy.sea_level_c(cell) - energy.surface_c(cell);
                assert!(
                    (drop - LAPSE_RATE * height / 1000.0).abs() < 0.01,
                    "cell {cell} at {height:.0} m was {drop:.1} °C cooler"
                );
                checked += 1;
            }
        }
        assert!(checked > 0, "the planet had no mountains to check");
    }

    #[test]
    fn carbon_dioxide_warms_it_logarithmically() {
        // Each doubling is worth the same, which is why the thermostat has to work
        // exponentially hard as the sun brightens.
        let planet = a_planet(0x5, 0.42);
        let at = |ppm| settled(&planet, 1.0, ppm).mean_c(&planet);
        let (a, b, c) = (at(150.0), at(300.0), at(600.0));
        let first = b - a;
        let second = c - b;
        assert!(
            first > 1.0,
            "a doubling of CO₂ was worth only {first:.2} °C"
        );
        assert!(
            (first - second).abs() < first * 0.45,
            "the two doublings were worth {first:.2} and {second:.2} °C"
        );
    }

    #[test]
    fn a_dim_enough_sun_freezes_it_solid() {
        // The ice–albedo runaway. Not a special case in the code: ice raises albedo,
        // which cools, which grows ice, and past a threshold the relaxation walks all
        // the way to a frozen planet on its own.
        let planet = a_planet(0x6, 0.42);
        let warm = settled(&planet, 1.0, REFERENCE_CO2_PPM);
        assert!(warm.ice_fraction(&planet) < 0.25);

        let frozen = settled(&planet, 0.70, REFERENCE_CO2_PPM);
        assert!(
            frozen.ice_fraction(&planet) > 0.9,
            "a sun at seven tenths left only {:.2} of the planet frozen",
            frozen.ice_fraction(&planet)
        );
        assert!(frozen.mean_c(&planet) < -20.0);
    }

    #[test]
    fn a_frozen_planet_stays_frozen_at_a_brightness_that_would_not_have_frozen_it() {
        // Hysteresis, and it is real: the ice line is stable in two places, and getting
        // out of a snowball takes far more than what it took to get in. This is the
        // reason the model relaxes from wherever the climate already was rather than
        // solving afresh each time.
        let planet = a_planet(0x7, 0.42);
        let mut energy = Energy::new(planet.grid());

        energy.solve(&planet, &sunlight(&planet, 0.65), REFERENCE_CO2_PPM, 300);
        assert!(
            energy.ice_fraction(&planet) > 0.9,
            "it should be a snowball"
        );

        // Back to a brightness that, coming from a warm planet, leaves nine tenths of
        // the surface unfrozen. Coming from a snowball it does nothing of the kind.
        let from_warm = settled(&planet, 0.95, REFERENCE_CO2_PPM).ice_fraction(&planet);
        energy.solve(&planet, &sunlight(&planet, 0.95), REFERENCE_CO2_PPM, 300);
        assert!(
            energy.ice_fraction(&planet) > from_warm * 1.5,
            "the snowball melted straight out to {:.2} ice, the same as a planet that \
             had never frozen ({from_warm:.2})",
            energy.ice_fraction(&planet)
        );
    }

    #[test]
    fn the_ocean_evens_out_what_the_land_does_not() {
        // Maritime against continental. Transport across water is several times what it
        // is across land, so a waterworld should have a much flatter gradient than a
        // planet that is mostly continent.
        let spread = |land| {
            let planet = a_planet(0x8, land);
            let energy = settled(&planet, 1.0, REFERENCE_CO2_PPM);
            let cells: Vec<f32> = planet
                .grid()
                .cells()
                .map(|c| energy.sea_level_c(c))
                .collect();
            let hot = cells.iter().copied().fold(f32::MIN, f32::max);
            let cold = cells.iter().copied().fold(f32::MAX, f32::min);
            hot - cold
        };
        let ocean = spread(0.05);
        let land = spread(0.80);
        assert!(
            land > ocean * 1.15,
            "a mostly-dry planet spanned {land:.0} °C against a wet one's {ocean:.0}"
        );
    }

    #[test]
    fn the_same_inputs_give_the_same_climate() {
        let planet = a_planet(0x9, 0.42);
        let once: Vec<f32> = {
            let e = settled(&planet, 1.0, REFERENCE_CO2_PPM);
            planet.grid().cells().map(|c| e.surface_c(c)).collect()
        };
        let twice: Vec<f32> = {
            let e = settled(&planet, 1.0, REFERENCE_CO2_PPM);
            planet.grid().cells().map(|c| e.surface_c(c)).collect()
        };
        assert_eq!(once, twice);
    }
}
