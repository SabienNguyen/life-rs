//! Where the rain falls, and therefore where the deserts are.
//!
//! Three things put a desert somewhere, and all three are here for their own reasons
//! rather than being drawn on:
//!
//! - **The subtropics.** Air rises at the equator, dries out as it rains, travels
//!   polewards aloft and comes back down around thirty degrees. Descending air warms and
//!   its relative humidity collapses, so it does not rain. That band is the Sahara, the
//!   Arabian, the Kalahari, the Atacama, and the Australian interior — a ring of desert
//!   around the planet at the same latitude in both hemispheres, which is not a
//!   coincidence anybody had to arrange.
//! - **Distance from the sea.** Moisture comes off water and is rained out as it travels,
//!   so a continental interior is dry for want of anything to carry rain to it.
//! - **Mountains.** Air forced up a range cools, drops its water on the windward side,
//!   and arrives on the other side dry. The lee is a rain shadow.
//!
//! Winds are prescribed by latitude rather than solved — trades, westerlies, polar
//! easterlies, with the vertical motion of the Hadley and Ferrel cells between them.
//! That is a parameterisation and the honest name for it, but it is a parameterisation of
//! the *cause*: the cells exist because of differential heating and rotation, they are
//! stable features of any rapidly rotating planet, and what follows from them here —
//! where the moisture goes and where it is dropped — is worked out rather than assumed.

use geo::{CellId, Grid, Lithosphere};

use crate::sphere::tangent_towards;

/// Rain in the reference state, in millimetres a year. Earth's global mean.
pub const REFERENCE_RAIN_MM: f32 = 1000.0;

/// How much water the air can hold, relative to fifteen degrees.
///
/// Clausius–Clapeyron: seven percent more per degree. It is the reason a warm planet has
/// a more vigorous water cycle, and the reason a frozen one has almost none.
pub fn saturation_ratio(temp_c: f32) -> f32 {
    const SLOPE: f32 = 17.67;
    const OFFSET: f32 = 243.5;
    let at = |t: f32| (SLOPE * t / (t + OFFSET)).exp();
    at(temp_c.clamp(-60.0, 60.0)) / at(15.0)
}

/// Vertical motion of the mean circulation at a latitude: positive rising, negative
/// sinking.
///
/// The Hadley, Ferrel and polar cells, written down. Rising at the equator, sinking
/// around thirty, rising again near sixty where the mid-latitude storm track lives, and
/// sinking over the poles — which is why the Antarctic is a desert by rainfall.
pub fn overturning(latitude_deg: f64) -> f32 {
    let phi = latitude_deg.abs();
    let cell = |centre: f64, width: f64| (-((phi - centre) / width).powi(2)).exp();
    (cell(0.0, 12.0) - 0.85 * cell(28.0, 11.0) + 0.55 * cell(58.0, 13.0) - 0.5 * cell(90.0, 15.0))
        as f32
}

/// The prevailing wind at a latitude, as (eastward, northward) components.
///
/// Easterly trades in the tropics, westerlies in the mid-latitudes, easterlies again at
/// the poles, with a meridional component towards whichever branch of the circulation
/// the air belongs to.
pub fn prevailing_wind(latitude_deg: f64) -> (f32, f32) {
    let phi = latitude_deg;
    let north = if phi >= 0.0 { 1.0 } else { -1.0 };
    let abs = phi.abs();
    let (zonal, meridional) = if abs < 30.0 {
        // Trades: blowing towards the west and towards the equator.
        (-1.0, -0.45)
    } else if abs < 60.0 {
        // Westerlies: towards the east and polewards.
        (1.0, 0.35)
    } else {
        (-0.7, -0.3)
    };
    (zonal as f32, (meridional * north) as f32)
}

/// The moisture field over a grid, and the rainfall it produces.
pub struct Moisture {
    humidity: Vec<f32>,
    scratch: Vec<f32>,
    rain_mm: Vec<f32>,
    /// Downwind share of each neighbour, flattened by cell.
    downwind: Vec<f32>,
    lift: Vec<f32>,
}

/// How much of the air's water is dropped per step of travel, at rest.
const BASE_RAINOUT: f32 = 0.16;
/// How much rising air adds to that, and sinking air takes away.
const OVERTURNING_EFFECT: f32 = 0.55;
/// How much being pushed uphill adds, per kilometre of climb across a cell.
const OROGRAPHIC: f32 = 0.55;
/// How fast water leaves the surface, relative to the reference.
const EVAPORATION: f32 = 1.0;

impl Moisture {
    pub fn new(cells: usize) -> Moisture {
        Moisture {
            humidity: vec![0.0; cells],
            scratch: vec![0.0; cells],
            rain_mm: vec![0.0; cells],
            downwind: vec![0.0; cells * geo::grid::MAX_NEIGHBOURS],
            lift: vec![0.0; cells * geo::grid::MAX_NEIGHBOURS],
        }
    }

    pub fn rain_mm(&self, cell: CellId) -> f32 {
        self.rain_mm[cell as usize]
    }

    /// Rainfall relative to the reference planet's mean — the units weathering wants.
    pub fn runoff(&self, cell: CellId) -> f32 {
        self.rain_mm[cell as usize] / REFERENCE_RAIN_MM
    }

    /// Carry moisture downwind until the field stops changing.
    ///
    /// Iterative rather than solved: what a cell receives depends on what its upwind
    /// neighbours have left after raining, which depends on what *they* received. A few
    /// dozen rounds is far past the point where the pattern stops moving, because
    /// moisture rarely survives more than a handful of cells of travel.
    pub fn settle(&mut self, planet: &Lithosphere, temp_c: &[f32], rounds: usize) {
        let grid = planet.grid();
        self.route(grid, planet);

        self.humidity.fill(0.0);
        for _ in 0..rounds {
            self.scratch.fill(0.0);
            for cell in grid.cells() {
                let i = cell as usize;

                // Water leaves a warm ocean readily, a cold one slowly, and land only
                // to the extent it has any — which is what makes a dry region stay dry
                // once it is dry.
                let wetness = if planet.is_land(cell) {
                    (self.rain_mm[i] / REFERENCE_RAIN_MM).min(1.0) * 0.45
                } else {
                    1.0
                };
                let frozen = temp_c[i] < crate::energy::ICE_POINT;
                let evaporation = if frozen {
                    0.02
                } else {
                    EVAPORATION * saturation_ratio(temp_c[i]) * wetness
                };

                let carried = self.humidity[i] + evaporation;
                let fraction = self.rainout(grid, cell, temp_c[i]);
                let fallen = carried * fraction;
                self.rain_mm[i] = fallen * REFERENCE_RAIN_MM;

                let onward = carried - fallen;
                let base = i * geo::grid::MAX_NEIGHBOURS;
                for (slot, &n) in grid.neighbours(cell).iter().enumerate() {
                    self.scratch[n as usize] += onward * self.downwind[base + slot];
                }
            }
            self.humidity.copy_from_slice(&self.scratch);
        }

        // One last pass so the reported rainfall matches the settled humidity rather
        // than the second-to-last round's.
        for cell in grid.cells() {
            let i = cell as usize;
            let wetness = if planet.is_land(cell) {
                (self.rain_mm[i] / REFERENCE_RAIN_MM).min(1.0) * 0.45
            } else {
                1.0
            };
            let evaporation = if temp_c[i] < crate::energy::ICE_POINT {
                0.02
            } else {
                EVAPORATION * saturation_ratio(temp_c[i]) * wetness
            };
            let carried = self.humidity[i] + evaporation;
            self.rain_mm[i] = carried * self.rainout(grid, cell, temp_c[i]) * REFERENCE_RAIN_MM;
        }
    }

    /// What share of the air's water falls out here.
    fn rainout(&self, grid: &Grid, cell: CellId, temp_c: f32) -> f32 {
        let latitude = grid.position(cell).latitude().to_degrees();
        let rising = overturning(latitude);

        // The steepest climb the wind is being forced into anywhere downwind of here.
        let base = cell as usize * geo::grid::MAX_NEIGHBOURS;
        let mut climb = 0.0f32;
        for slot in 0..grid.degree(cell) {
            let share = self.downwind[base + slot];
            if share > 0.0 {
                climb = climb.max(self.lift[base + slot] * share);
            }
        }

        // Cold air holds little, so what it has falls out readily; warm air holds more
        // and can carry it further inland.
        let holding = saturation_ratio(temp_c).clamp(0.15, 4.0);
        let fraction = (BASE_RAINOUT / holding.sqrt()) * (1.0 + OVERTURNING_EFFECT * rising)
            + OROGRAPHIC * climb;
        fraction.clamp(0.01, 0.97)
    }

    /// Work out, for every cell, how its air is shared among its neighbours and how far
    /// uphill each of those directions is.
    fn route(&mut self, grid: &Grid, planet: &Lithosphere) {
        for cell in grid.cells() {
            let here = grid.position(cell);
            let latitude = here.latitude().to_degrees();
            let (east, north) = prevailing_wind(latitude);

            // The wind as a direction on the sphere at this point.
            let up = geo::Vec3::new(0.0, 0.0, 1.0);
            let eastward = up.cross(here).normalised();
            let northward = here.cross(eastward).normalised();
            let wind = eastward
                .scaled(east as f64)
                .plus(northward.scaled(north as f64))
                .normalised();

            let base = cell as usize * geo::grid::MAX_NEIGHBOURS;
            let mut total = 0.0f32;
            for (slot, &n) in grid.neighbours(cell).iter().enumerate() {
                let towards = tangent_towards(here, grid.position(n));
                // Only the downwind half of the neighbours receive anything, weighted by
                // how squarely they lie in the wind's path.
                let share = towards.dot(wind).max(0.0) as f32;
                self.downwind[base + slot] = share;
                total += share;

                let climb = (planet.height_above_sea_m(n).max(0.0)
                    - planet.height_above_sea_m(cell).max(0.0))
                    / 1000.0;
                self.lift[base + slot] = climb.max(0.0);
            }
            if total > 0.0 {
                for slot in 0..grid.degree(cell) {
                    self.downwind[base + slot] /= total;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::energy::Energy;
    use crate::insolation::{EARTH_OBLIQUITY, SOLAR_CONSTANT, annual_mean};
    use sim_core::{Domain, WorldSeed};

    fn a_world(seed: u128, land: f32) -> (Lithosphere, Energy, Moisture) {
        let mut rng = WorldSeed::from_u128(seed).stream(Domain::Terrain, 0, 0);
        let planet = Lithosphere::genesis(4, 9, land, &mut rng);
        let grid = planet.grid();
        let sun: Vec<f32> = grid
            .cells()
            .map(|c| {
                annual_mean(grid.position(c).latitude(), EARTH_OBLIQUITY, SOLAR_CONSTANT) as f32
            })
            .collect();
        let mut energy = Energy::new(grid);
        energy.solve(&planet, &sun, 300.0, 250);

        let temps: Vec<f32> = grid.cells().map(|c| energy.surface_c(c)).collect();
        let mut moisture = Moisture::new(grid.len());
        moisture.settle(&planet, &temps, 60);
        (planet, energy, moisture)
    }

    fn band_rain(planet: &Lithosphere, moisture: &Moisture, low: f64, high: f64) -> f32 {
        let cells: Vec<CellId> = planet
            .grid()
            .cells()
            .filter(|c| {
                let lat = planet.grid().position(*c).latitude().to_degrees().abs();
                (low..high).contains(&lat)
            })
            .collect();
        cells.iter().map(|c| moisture.rain_mm(*c)).sum::<f32>() / cells.len() as f32
    }

    #[test]
    fn warm_air_holds_more_water() {
        assert!(saturation_ratio(25.0) > saturation_ratio(15.0));
        assert!(saturation_ratio(5.0) < saturation_ratio(15.0));
        assert!((saturation_ratio(15.0) - 1.0).abs() < 1e-6);
        // Seven percent a degree, near the reference.
        let per_degree = saturation_ratio(16.0) / saturation_ratio(15.0);
        assert!(
            (per_degree - 1.07).abs() < 0.01,
            "a degree was worth {per_degree:.3}"
        );
    }

    #[test]
    fn the_circulation_rises_at_the_equator_and_sinks_at_thirty() {
        assert!(overturning(0.0) > 0.5, "the equator should be rising");
        assert!(overturning(28.0) < -0.4, "the subtropics should be sinking");
        assert!(overturning(-28.0) < -0.4, "and in the south too");
        assert!(overturning(58.0) > 0.2, "the storm track should be rising");
        assert!(overturning(90.0) < 0.0, "the poles should be sinking");
    }

    #[test]
    fn winds_reverse_between_the_tropics_and_the_mid_latitudes() {
        assert!(prevailing_wind(15.0).0 < 0.0, "trades blow to the west");
        assert!(prevailing_wind(45.0).0 > 0.0, "westerlies blow to the east");
        assert!(prevailing_wind(75.0).0 < 0.0, "polar easterlies");
        // And the meridional component flips across the equator.
        assert!(prevailing_wind(15.0).1 < 0.0);
        assert!(prevailing_wind(-15.0).1 > 0.0);
    }

    #[test]
    fn there_is_a_belt_of_desert_in_the_subtropics() {
        // The headline result. Nothing places a desert; the subtropics are dry because
        // that is where the air is coming down.
        let (planet, _, moisture) = a_world(0x1, 0.42);
        let equator = band_rain(&planet, &moisture, 0.0, 10.0);
        let subtropics = band_rain(&planet, &moisture, 20.0, 35.0);
        let midlatitudes = band_rain(&planet, &moisture, 45.0, 60.0);

        assert!(
            equator > subtropics * 1.4,
            "the equator had {equator:.0} mm against the subtropics' {subtropics:.0}"
        );
        assert!(
            midlatitudes > subtropics * 1.1,
            "the storm track had {midlatitudes:.0} mm against {subtropics:.0}"
        );
    }

    #[test]
    fn the_poles_are_deserts_too() {
        // By rainfall, which is the sense in which the Antarctic is one of the driest
        // places on the planet.
        let (planet, _, moisture) = a_world(0x2, 0.42);
        let poles = band_rain(&planet, &moisture, 75.0, 91.0);
        let equator = band_rain(&planet, &moisture, 0.0, 10.0);
        assert!(
            poles < equator * 0.4,
            "the poles had {poles:.0} mm against the equator's {equator:.0}"
        );
    }

    #[test]
    fn the_far_side_of_a_mountain_is_dry() {
        // A rain shadow, found by comparing what falls on cells being pushed uphill
        // with what falls on cells being pushed down, at the same latitude band.
        let (planet, _, moisture) = a_world(0x3, 0.5);
        let grid = planet.grid();

        let mut windward = (0.0f32, 0usize);
        let mut lee = (0.0f32, 0usize);
        for cell in grid.cells() {
            if !planet.is_land(cell) {
                continue;
            }
            let lat = grid.position(cell).latitude().to_degrees().abs();
            if !(35.0..60.0).contains(&lat) {
                continue;
            }
            // Westerlies here, so the upwind neighbour is the one to the west.
            let here = grid.position(cell);
            let up = geo::Vec3::new(0.0, 0.0, 1.0);
            let eastward = up.cross(here).normalised();
            let upwind = grid
                .neighbours(cell)
                .iter()
                .min_by(|a, b| {
                    tangent_towards(here, grid.position(**a))
                        .dot(eastward)
                        .total_cmp(&tangent_towards(here, grid.position(**b)).dot(eastward))
                })
                .copied()
                .unwrap();
            let climb = planet.height_above_sea_m(cell) - planet.height_above_sea_m(upwind);
            if climb > 700.0 {
                windward.0 += moisture.rain_mm(cell);
                windward.1 += 1;
            } else if climb < -700.0 {
                lee.0 += moisture.rain_mm(cell);
                lee.1 += 1;
            }
        }
        assert!(
            windward.1 > 2 && lee.1 > 2,
            "not enough relief to test: {} up, {} down",
            windward.1,
            lee.1
        );
        let up = windward.0 / windward.1 as f32;
        let down = lee.0 / lee.1 as f32;
        assert!(
            up > down * 1.2,
            "climbing cells got {up:.0} mm and descending ones {down:.0}"
        );
    }

    #[test]
    fn the_middle_of_a_continent_is_drier_than_its_coast() {
        let (planet, _, moisture) = a_world(0x4, 0.6);
        let grid = planet.grid();

        // How far each land cell is from the sea, in hops.
        let mut inland = vec![u32::MAX; grid.len()];
        let mut queue: Vec<CellId> = grid.cells().filter(|c| !planet.is_land(*c)).collect();
        for c in &queue {
            inland[*c as usize] = 0;
        }
        let mut at = 0;
        while at < queue.len() {
            let cell = queue[at];
            at += 1;
            for &n in grid.neighbours(cell) {
                if inland[n as usize] == u32::MAX {
                    inland[n as usize] = inland[cell as usize] + 1;
                    queue.push(n);
                }
            }
        }

        let mean_at = |hops: u32| {
            let cells: Vec<CellId> = grid
                .cells()
                .filter(|c| inland[*c as usize] == hops)
                .collect();
            (
                cells.iter().map(|c| moisture.rain_mm(*c)).sum::<f32>() / cells.len() as f32,
                cells.len(),
            )
        };
        let (coast, n1) = mean_at(1);
        let (deep, n2) = mean_at(4);
        assert!(
            n1 > 5 && n2 > 5,
            "not enough continent: {n1} and {n2} cells"
        );
        assert!(
            coast > deep * 1.15,
            "the coast had {coast:.0} mm and the interior {deep:.0}"
        );
    }

    #[test]
    fn a_frozen_planet_barely_rains() {
        // The water cycle shuts down when there is no liquid water to lift. This is why
        // a snowball is so hard to get out of by weathering — the rain that would draw
        // carbon down is not falling.
        let mut rng = WorldSeed::from_u128(0x5).stream(Domain::Terrain, 0, 0);
        let planet = Lithosphere::genesis(4, 9, 0.42, &mut rng);
        let grid = planet.grid();
        let frozen = vec![-40.0f32; grid.len()];
        let mut moisture = Moisture::new(grid.len());
        moisture.settle(&planet, &frozen, 60);
        let mean: f32 = grid.cells().map(|c| moisture.rain_mm(c)).sum::<f32>() / grid.len() as f32;
        assert!(mean < 60.0, "a snowball rained {mean:.0} mm a year");
    }

    #[test]
    fn a_warm_planet_has_a_more_vigorous_water_cycle() {
        let mut rng = WorldSeed::from_u128(0x6).stream(Domain::Terrain, 0, 0);
        let planet = Lithosphere::genesis(4, 9, 0.42, &mut rng);
        let grid = planet.grid();
        let mean_rain = |offset: f32| {
            let temps: Vec<f32> = grid
                .cells()
                .map(|c| 15.0 + offset - 30.0 * grid.position(c).latitude().abs() as f32)
                .collect();
            let mut moisture = Moisture::new(grid.len());
            moisture.settle(&planet, &temps, 60);
            grid.cells().map(|c| moisture.rain_mm(c)).sum::<f32>() / grid.len() as f32
        };
        assert!(mean_rain(10.0) > mean_rain(0.0) * 1.2);
    }

    #[test]
    fn the_global_mean_is_in_the_right_range() {
        let (planet, _, moisture) = a_world(0x7, 0.42);
        let grid = planet.grid();
        let mean: f32 = grid.cells().map(|c| moisture.rain_mm(c)).sum::<f32>() / grid.len() as f32;
        assert!(
            (400.0..2000.0).contains(&mean),
            "the planet averaged {mean:.0} mm of rain a year"
        );
    }
}
