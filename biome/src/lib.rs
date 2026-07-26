//! The living skin of the planet — and the point at which it stops being a plan.
//!
//! There is no data in this crate. A biome is a *reading* of two numbers the climate
//! already computes, and productivity is a reading of the same two. That is design
//! principle two doing its work: store the vectors, derive the labels. The consequence is
//! the interesting part — because nothing is stored, nothing has to be kept in step, and
//! **biomes move on their own**. An orogeny casts a rain shadow and the forest behind it
//! becomes steppe over a few hundred thousand years. A continent drifts into the
//! subtropics and grows a desert down its middle. The sun brightens, the thermostat draws
//! carbon down, and the tundra retreats towards the pole. Nobody edits a biome map,
//! because there is no biome map — there is a planet, a climate, and a function.
//!
//! What is not here: individual plants, and any notion of dispersal or competition
//! between them. A cell's vegetation is whatever its climate implies, arrived at
//! instantly. Real vegetation lags its climate by centuries and can hold ground it would
//! not win from scratch, and the machinery for that — plant functional types with their
//! own populations — belongs with the ecology phase.

pub mod productivity;
pub mod whittaker;

use climate::Climate;
use geo::{CellId, Lithosphere};
use ocean::Ocean;

pub use whittaker::{Biome, SHELF_DEPTH_M};

/// What lives where, and how much of it.
pub struct Biosphere {
    biome: Vec<Biome>,
    /// Net primary production, grams of dry matter per square metre a year.
    production: Vec<f32>,
    /// The sea this was read against.
    ///
    /// Kept rather than taken as an argument because it is the same kind of thing as the
    /// rest of this struct — a reading of the planet and the climate, with no state of its
    /// own — and because everything that wants a biosphere also wants the sea it implies.
    /// Building it here means one place constructs it and one place is responsible for it
    /// matching.
    sea: Ocean,
}

impl Biosphere {
    /// Read the biosphere off a planet and its climate.
    ///
    /// Cheap enough to do every step and not worth caching: it is two arithmetic
    /// expressions per cell with no iteration and no state.
    pub fn read(planet: &Lithosphere, climate: &Climate) -> Biosphere {
        let grid = planet.grid();
        let mut biome = Vec::with_capacity(grid.len());
        let mut production = Vec::with_capacity(grid.len());
        let inland = continentality(planet);
        let sea = Ocean::read(planet, climate);

        for cell in grid.cells() {
            let latitude = grid.position(cell).latitude();
            let under_water = !planet.is_land(cell);
            let depth = (-planet.height_above_sea_m(cell)).max(0.0);
            let mean = climate.temperature_c(cell);
            let warmest =
                whittaker::warmest_month_c(mean, latitude.to_degrees(), inland[cell as usize]);
            let rain = climate.rain_mm(cell);

            let kind = whittaker::classify(mean, warmest, rain, under_water, depth);
            let sunlight = climate::insolation::annual_mean(
                latitude,
                climate.obliquity_deg(),
                climate::insolation::SOLAR_CONSTANT * climate.brightness(),
            ) as f32;

            production.push(productivity::of(
                kind,
                mean,
                rain,
                sunlight,
                depth < SHELF_DEPTH_M,
                sea.nutrients(cell),
            ));
            biome.push(kind);
        }
        Biosphere {
            biome,
            production,
            sea,
        }
    }

    /// The sea this biosphere was read against.
    pub fn sea(&self) -> &Ocean {
        &self.sea
    }

    pub fn biome(&self, cell: CellId) -> Biome {
        self.biome[cell as usize]
    }

    pub fn production(&self, cell: CellId) -> f32 {
        self.production[cell as usize]
    }

    /// Share of the surface each biome covers, indexed the same way [`Biome`] is.
    pub fn shares(&self, planet: &Lithosphere) -> [f32; Biome::COUNT] {
        let grid = planet.grid();
        let mut shares = [0.0f64; Biome::COUNT];
        let mut total = 0.0;
        for cell in grid.cells() {
            let area = grid.solid_angle(cell);
            shares[self.biome(cell) as usize] += area;
            total += area;
        }
        let mut out = [0.0f32; Biome::COUNT];
        for (slot, area) in shares.iter().enumerate() {
            out[slot] = (area / total) as f32;
        }
        out
    }

    /// Everything the biosphere makes in a year, in gigatonnes of dry matter.
    ///
    /// The one number that says how alive a planet is. Earth's is around a hundred and
    /// twenty, split about evenly between the land and the sea.
    pub fn total_production_gt(&self, planet: &Lithosphere) -> f32 {
        let grid = planet.grid();
        let mut total = 0.0;
        for cell in grid.cells() {
            // Cell area in square kilometres, production in grams a square metre: a
            // million square metres to the square kilometre, and 10¹⁵ grams to the
            // gigatonne.
            let area_m2 = grid.area_km2(cell, geo::EARTH_RADIUS_KM) * 1.0e6;
            total += self.production(cell) as f64 * area_m2;
        }
        (total / 1.0e15) as f32
    }

    /// Production on land alone, in gigatonnes a year.
    pub fn land_production_gt(&self, planet: &Lithosphere) -> f32 {
        let grid = planet.grid();
        let mut total = 0.0;
        for cell in grid.cells() {
            if self.biome(cell).is_marine() {
                continue;
            }
            total +=
                self.production(cell) as f64 * grid.area_km2(cell, geo::EARTH_RADIUS_KM) * 1.0e6;
        }
        (total / 1.0e15) as f32
    }

    /// Share of the land that carries forest — the number a deep-time view watches,
    /// because it moves with every glaciation and every mountain range.
    pub fn forest_share(&self, planet: &Lithosphere) -> f32 {
        let grid = planet.grid();
        let mut forest = 0.0;
        let mut land = 0.0;
        for cell in grid.cells() {
            if self.biome(cell).is_marine() {
                continue;
            }
            let area = grid.solid_angle(cell);
            land += area;
            if self.biome(cell).is_forest() {
                forest += area;
            }
        }
        if land == 0.0 {
            return 0.0;
        }
        (forest / land) as f32
    }

    /// Share of the land too dry to hold much of anything.
    pub fn desert_share(&self, planet: &Lithosphere) -> f32 {
        let grid = planet.grid();
        let mut arid = 0.0;
        let mut land = 0.0;
        for cell in grid.cells() {
            if self.biome(cell).is_marine() {
                continue;
            }
            let area = grid.solid_angle(cell);
            land += area;
            if self.biome(cell).is_arid() {
                arid += area;
            }
        }
        if land == 0.0 {
            return 0.0;
        }
        (arid / land) as f32
    }
}

/// How far from the sea each cell is, from nought at the coast to one deep inland.
///
/// Continentality, and it is worth computing rather than guessing because it is half of
/// what decides whether a cold place grows a forest. Measured in hops across the cell
/// graph and saturating after a few, which is about right: the moderating reach of an
/// ocean is a few hundred kilometres, not a few thousand.
fn continentality(planet: &Lithosphere) -> Vec<f32> {
    const REACH: f32 = 4.0;
    let grid = planet.grid();
    let mut hops = vec![u32::MAX; grid.len()];
    let mut queue: Vec<CellId> = grid.cells().filter(|c| !planet.is_land(*c)).collect();
    for cell in &queue {
        hops[*cell as usize] = 0;
    }
    let mut at = 0;
    while at < queue.len() {
        let cell = queue[at];
        at += 1;
        let next = hops[cell as usize] + 1;
        for &n in grid.neighbours(cell) {
            if hops[n as usize] == u32::MAX {
                hops[n as usize] = next;
                queue.push(n);
            }
        }
    }
    // A planet with no ocean at all: everywhere is equally interior.
    hops.iter()
        .map(|h| {
            if *h == u32::MAX {
                1.0
            } else {
                (*h as f32 / REACH).min(1.0)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
