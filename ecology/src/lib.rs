//! Animals, as populations spread over the map rather than as individuals.
//!
//! A **deme** is how much of one species lives in one cell. That is the right grain for
//! deep time: over a megayear the questions are which species exist, where their ranges
//! reach, and which of them are gone — and none of those need a single animal to be
//! represented anywhere. A wildebeest is not an entity here. Four million wildebeest are
//! a number in a cell, and the herd only becomes individuals if somebody zooms in far
//! enough for the level-of-detail machinery to make them.
//!
//! What holds the whole thing up is the productivity field the biosphere already
//! computes. Plants make a certain amount of matter a year; some fraction of it is
//! edible; a tenth of what is eaten becomes herbivore; a tenth of *that* becomes
//! carnivore. So the pyramid is not asserted — it is what the arithmetic does, and its
//! shape follows the vegetation around as the vegetation follows the climate.
//!
//! ## What this is not
//!
//! There is no individual behaviour, no age structure, no seasonal migration, and no
//! evolution — species are fixed once drawn, and go extinct but never change or split.
//! Speciation and selection are the next phase and they need this one to exist first,
//! because what evolution acts on is exactly these ranges and these population sizes.

pub mod species;

use biome::Biosphere;
use geo::{CellId, Lithosphere};
use sim_core::{Domain, Rng, WorldSeed};

pub use species::{Species, Trophic};

/// A handle on a species. Plain index: species are appended and never moved.
pub type SpeciesId = u16;

/// How much of a population survives a megayear in a place that cannot support it.
///
/// Not zero, because a range contracts to refuges rather than vanishing at a stroke, and
/// a refuge is what lets a species come back when the climate turns.
const DECLINE: f32 = 0.25;
/// How fast a population climbs towards what its cell can support.
///
/// Nearly all the way in one step, and that is right at this grain: a population doubles
/// in years and the step is a megayear. What actually limits a range over deep time is
/// dispersal and tolerance, not how fast anything breeds.
const RECOVERY: f32 = 0.85;
/// Total biomass, in tonnes, below which a species is called extinct.
const EXTINCTION_FLOOR: f32 = 500.0;
/// Below this density, in tonnes per square kilometre, a species is a straggler rather
/// than a resident.
///
/// An absolute figure and not a share of the species' own best cell, which is what it was
/// first and which quietly inverted the diversity map: a species with an enormous tropical
/// peak counted as absent everywhere else, so the richest places came out looking the
/// poorest.
const PRESENCE_T_PER_KM2: f32 = 0.05;

/// How rare a species has to be, against the average of its living peers, before chance
/// starts to matter to it.
///
/// Relative rather than a fixed tonnage, so that it means the same thing on a lush planet
/// and a barren one. A fixed figure was tried first and was simply never reached: every
/// species on a productive world holds millions of tonnes, and nothing ever died.
const FRAGILE_SHARE: f32 = 0.16;
/// A range smaller than this many cells is a range one bad megayear can finish.
///
/// Range size is the best single predictor of extinction risk there is, better than
/// abundance and far better than body size, and it is what balances a fauna: more species
/// means smaller ranges each, which means more of them die, which is what stops
/// origination running away to a ceiling. Without it a planet whose geography splits
/// ranges readily accumulates species without limit.
const SAFE_RANGE_CELLS: f32 = 24.0;

/// The chance a fragile species is gone within a megayear, at its most fragile.
///
/// Demographic stochasticity: a small population can be finished by a bad century that a
/// large one would ride out. Without it nothing ever dies here — every species keeps some
/// refuge somewhere, holds a few hundred tonnes in it forever, and the planet accumulates
/// species without limit.
const CHANCE_OF_LOSS: f64 = 0.10;

/// Every animal population on the planet.
pub struct Ecology {
    species: Vec<Species>,
    /// Living biomass in tonnes, one row per species, one entry per cell.
    biomass: Vec<Vec<f32>>,
    extinct: Vec<Option<f64>>,
    scratch: Vec<f32>,
    spread: Vec<f32>,
    /// How well the species being stepped is suited to each cell.
    suits: Vec<f32>,
    /// Total herbivore biomass per cell — what the carnivores are eating.
    prey: Vec<f32>,
    /// Cell areas, kept because presence is a density and densities need them.
    area_km2: Vec<f32>,
    /// Total claim on each cell, by class of animal. Four classes: land and sea, each
    /// eating plants or eating each other.
    contested: [Vec<f32>; 4],
    age_myr: f64,
    /// Extinctions and originations, for the record.
    pub lost: usize,
    pub gained: usize,
}

impl Ecology {
    /// Stock a planet with animals.
    ///
    /// Herbivores outnumber carnivores three to one, and both are split between land and
    /// sea in proportion to how much of each there is. Whether any given species survives
    /// its first megayear is not decided here: it is drawn, put down everywhere it can
    /// live, and left to the arithmetic.
    pub fn genesis(
        planet: &Lithosphere,
        life: &Biosphere,
        climate: &climate::Climate,
        count: usize,
        seed: WorldSeed,
    ) -> Ecology {
        let mut rng = seed.stream(Domain::Ecology, 0, 0);
        let cells = planet.grid().len();
        let wet = 1.0 - planet.land_fraction() as f64;

        let mut ecology = Ecology {
            species: Vec::with_capacity(count),
            biomass: Vec::with_capacity(count),
            extinct: Vec::with_capacity(count),
            scratch: vec![0.0; cells],
            spread: vec![0.0; cells],
            suits: vec![0.0; cells],
            prey: vec![0.0; cells],
            contested: [
                vec![0.0; cells],
                vec![0.0; cells],
                vec![0.0; cells],
                vec![0.0; cells],
            ],
            area_km2: planet
                .grid()
                .cells()
                .map(|c| planet.grid().area_km2(c, geo::EARTH_RADIUS_KM) as f32)
                .collect(),
            age_myr: 0.0,
            lost: 0,
            gained: 0,
        };

        // Where species come from. Not uniformly across the thermometer — a lineage
        // arises where there is something to eat, so the temperatures new species are
        // built around are drawn from the planet's own, weighted by how much each place
        // produces. That is the energy hypothesis for why the tropics are diverse, and
        // making it the *cause* here is the honest way to get the pattern: nothing counts
        // latitudes, it counts calories.
        let opportunity = warm_places(planet, life, climate);

        for i in 0..count {
            let trophic = if i % 4 == 3 {
                Trophic::Carnivore
            } else {
                Trophic::Herbivore
            };
            let marine = rng.chance(wet);
            let centre = *rng.pick(&opportunity).unwrap_or(&15.0);
            let mut species = Species::around(centre, trophic, marine, 0.0, &mut rng);
            species.name = name_for(&species, i, &mut rng);
            ecology.introduce(planet, life, species);
        }
        ecology
    }

    /// Add a species and seed it wherever it can live.
    pub fn introduce(
        &mut self,
        planet: &Lithosphere,
        life: &Biosphere,
        species: Species,
    ) -> SpeciesId {
        let grid = planet.grid();
        let mut row = vec![0.0f32; grid.len()];
        // A token presence everywhere suitable, so the first step's growth has something
        // to work on. Introducing a species into exactly one cell would make its fate
        // depend on which cell, which is a different simulation.
        for cell in grid.cells() {
            if species.suitability(0.0, 0.0, life.biome(cell)) > 0.0 {
                row[cell as usize] = 1.0;
            }
        }
        self.species.push(species);
        self.biomass.push(row);
        self.extinct.push(None);
        self.gained += 1;
        (self.species.len() - 1) as SpeciesId
    }

    // ---- reading it ------------------------------------------------------------

    pub fn species(&self) -> &[Species] {
        &self.species
    }

    pub fn get(&self, id: SpeciesId) -> &Species {
        &self.species[id as usize]
    }

    pub fn is_extinct(&self, id: SpeciesId) -> bool {
        self.extinct[id as usize].is_some()
    }

    /// When this species died out, in megayears since the world began.
    pub fn extinct_at(&self, id: SpeciesId) -> Option<f64> {
        self.extinct[id as usize]
    }

    /// Living species, by handle.
    pub fn living(&self) -> impl Iterator<Item = SpeciesId> + '_ {
        (0..self.species.len() as SpeciesId).filter(|id| !self.is_extinct(*id))
    }

    pub fn richness(&self) -> usize {
        self.living().count()
    }

    /// Biomass of one species in one cell, in tonnes.
    pub fn biomass_of(&self, id: SpeciesId, cell: CellId) -> f32 {
        self.biomass[id as usize][cell as usize]
    }

    /// How many species are present in a cell — the local richness, and the thing that
    /// makes a latitudinal diversity gradient visible.
    pub fn richness_at(&self, cell: CellId) -> usize {
        self.living()
            .filter(|id| self.biomass_of(*id, cell) > self.presence_floor(cell))
            .count()
    }

    /// The least biomass that counts as living in a cell, given its size.
    pub fn presence_floor(&self, cell: CellId) -> f32 {
        self.area_km2[cell as usize] * PRESENCE_T_PER_KM2
    }

    /// Total living animal biomass on the planet, in millions of tonnes.
    pub fn total_biomass_mt(&self) -> f32 {
        let mut total = 0.0f64;
        for id in self.living() {
            total += self.biomass[id as usize]
                .iter()
                .map(|b| *b as f64)
                .sum::<f64>();
        }
        (total / 1.0e6) as f32
    }

    /// Total biomass at one level of the food chain, in millions of tonnes.
    pub fn biomass_at_mt(&self, trophic: Trophic) -> f32 {
        let mut total = 0.0f64;
        for id in self.living().filter(|id| self.get(*id).trophic == trophic) {
            total += self.biomass[id as usize]
                .iter()
                .map(|b| *b as f64)
                .sum::<f64>();
        }
        (total / 1.0e6) as f32
    }

    /// How many cells a species occupies — the size of its range.
    pub fn range_of(&self, id: SpeciesId) -> usize {
        self.biomass[id as usize]
            .iter()
            .enumerate()
            .filter(|(cell, held)| **held > self.area_km2[*cell] * PRESENCE_T_PER_KM2)
            .count()
    }

    /// Move a species' whole tolerance band, keeping its width.
    ///
    /// The one thing outside this crate is allowed to change about a species, and it is
    /// what adaptation is: the band follows the conditions the population is actually
    /// living in, and it follows slowly.
    pub fn shift_tolerance(&mut self, id: SpeciesId, by: f32) {
        let species = &mut self.species[id as usize];
        species.coldest_c += by;
        species.warmest_c += by;
    }

    /// Take part of a species' range away and make it a species of its own.
    ///
    /// Allopatry. The cells named become the daughter's whole range and cease to be the
    /// parent's — one population has become two, and neither is where the other is.
    pub fn split_off(
        &mut self,
        cells: &[CellId],
        from: SpeciesId,
        daughter: Species,
        life: &Biosphere,
        climate: &climate::Climate,
    ) -> SpeciesId {
        let mut row = vec![0.0f32; self.area_km2.len()];
        for cell in cells {
            row[*cell as usize] = self.biomass[from as usize][*cell as usize];
            self.biomass[from as usize][*cell as usize] = 0.0;
        }
        let _ = (life, climate);
        self.species.push(daughter);
        self.biomass.push(row);
        self.extinct.push(None);
        self.gained += 1;
        (self.species.len() - 1) as SpeciesId
    }

    // ---- running it ------------------------------------------------------------

    /// Advance every population by a span of megayears.
    ///
    /// Three things happen, in an order that matters: what each cell can support is
    /// recomputed from the vegetation as it now is, populations move towards it, and then
    /// what survives spreads into the cells next door. Dispersal last, so that a species
    /// pushed out of a region this step cannot immediately re-enter it from a neighbour
    /// that was also pushed out.
    pub fn step_myr(
        &mut self,
        planet: &Lithosphere,
        life: &Biosphere,
        climate: &climate::Climate,
        dt: f32,
        rng: &mut Rng,
    ) {
        debug_assert!(dt > 0.0, "time only runs forwards");
        self.age_myr += dt as f64;
        let grid = planet.grid();

        // What the herbivores amount to, per cell. The carnivores eat this, and it has to
        // be the state as it stood at the start of the step or a predator would be
        // feeding on prey that this same step already produced.
        self.prey.fill(0.0);
        for id in self.living().collect::<Vec<_>>() {
            if self.get(id).trophic != Trophic::Herbivore {
                continue;
            }
            for cell in 0..grid.len() {
                self.prey[cell] += self.biomass[id as usize][cell];
            }
        }

        // What counts as rare on this planet, at this moment.
        let alive = self.richness().max(1) as f32;
        let fragile_below =
            (self.total_biomass_mt() * 1.0e6 / alive * FRAGILE_SHARE).max(EXTINCTION_FLOOR);

        // What each cell's supply is being divided among. Proportional to how well each
        // competitor is suited, not shared out equally — equal shares was the first
        // version and it makes every species of a class come out the same size, so none
        // is ever rare and nothing ever dies. Sharing by fitness gives winners and losers,
        // and it is losers that extinction needs.
        self.demand_for_cells(planet, life, climate);

        for id in self.living().collect::<Vec<_>>() {
            let species = self.species[id as usize].clone();
            let class = class_of(&species);

            for cell in 0..grid.len() {
                let id_cell = cell as CellId;
                let suitability = species.suitability(
                    climate.temperature_c(id_cell),
                    climate.rain_mm(id_cell),
                    life.biome(id_cell),
                );

                self.suits[cell] = suitability;
                let contest = self.contested[class][cell].max(suitability);
                let capacity = if suitability <= 0.0 || contest <= 0.0 {
                    0.0
                } else {
                    self.capacity_tonnes(&species, cell, life, id_cell, self.area_km2[cell])
                        * suitability
                        / contest
                };

                let now = self.biomass[id as usize][cell];
                let step = if capacity > now {
                    now + (capacity - now) * RECOVERY
                } else {
                    // Falling towards what the place can now support, and not instantly:
                    // a shrinking range leaves relicts behind it.
                    capacity + (now - capacity) * DECLINE.powf(dt)
                };
                self.scratch[cell] = step.max(0.0);
            }

            // Spreading. A share of what sits in each cell tries the cells next door —
            // but only the ones it could actually live in. Animals do wander into ground
            // that cannot keep them; what they do not do is establish there, and letting
            // them meant finding grazers on glaciers.
            //
            // Built into a second buffer rather than in place: writing the result back
            // cell by cell overwrites the arrivals from neighbours not yet reached, and a
            // species then only ever disperses in one direction.
            let reach = (species.dispersal * dt).clamp(0.0, 0.6);
            self.spread.copy_from_slice(&self.scratch);
            for cell in 0..grid.len() {
                let leaving = self.scratch[cell] * reach;
                if leaving <= 0.0 {
                    continue;
                }
                let neighbours = grid.neighbours(cell as CellId);
                let open = neighbours
                    .iter()
                    .filter(|n| self.suits[**n as usize] > 0.0)
                    .count();
                if open == 0 {
                    continue;
                }
                let each = leaving / open as f32;
                self.spread[cell] -= leaving;
                for &n in neighbours {
                    if self.suits[n as usize] > 0.0 {
                        self.spread[n as usize] += each;
                    }
                }
            }
            self.biomass[id as usize].copy_from_slice(&self.spread);

            // Gone outright, or rare enough that a bad century finishes it. The second
            // is what produces a background extinction rate at all, and it is also what
            // turns a climate shift that squeezes many species at once into a mass
            // extinction rather than a slow thinning.
            let total: f32 = self.biomass[id as usize].iter().sum();
            let doomed = total < EXTINCTION_FLOOR || {
                let by_rarity = (1.0 - total / fragile_below).clamp(0.0, 1.0);
                let by_range = (1.0 - self.range_of(id) as f32 / SAFE_RANGE_CELLS).clamp(0.0, 1.0);
                // Whichever is worse. A species can be doomed by being scarce everywhere
                // or by being abundant nowhere in particular, and they are different
                // ways to go.
                let exposure = by_rarity.max(by_range) as f64;
                let odds = 1.0 - (1.0 - CHANCE_OF_LOSS * exposure).powf(dt as f64);
                rng.chance(odds)
            };
            if doomed {
                self.extinct[id as usize] = Some(self.age_myr);
                self.biomass[id as usize].iter_mut().for_each(|b| *b = 0.0);
                self.lost += 1;
            }
        }
    }

    /// What one cell could support of a species, if it had the place to itself.
    fn capacity_tonnes(
        &self,
        species: &Species,
        cell: usize,
        life: &Biosphere,
        id: CellId,
        area_km2: f32,
    ) -> f32 {
        // Grams a square metre a year, over the cell, in tonnes of dry matter.
        let area_m2 = area_km2 * 1.0e6;
        let energy_t = match species.trophic {
            Trophic::Herbivore => {
                let edible = life.production(id) * species::edible_share(life.biome(id));
                edible * area_m2 / 1.0e6 * species::TRANSFER
            }
            // A carnivore's supply is what the herbivores in its cell amount to, and the
            // same tenth of it gets through.
            Trophic::Carnivore => self.prey[cell] * species::TRANSFER,
        };
        // Standing biomass against annual throughput: a population turns over roughly
        // once a year at small sizes and far more slowly at large ones, which is the
        // other half of why big animals are rare.
        let turnover = (species.mass_kg.max(0.001)).powf(0.25) * 0.6;
        energy_t * turnover
    }

    /// For every cell and every class of animal, how much total claim is being made on
    /// it — the denominator each competitor's share is divided by.
    fn demand_for_cells(
        &mut self,
        planet: &Lithosphere,
        life: &Biosphere,
        climate: &climate::Climate,
    ) {
        let grid = planet.grid();
        for row in self.contested.iter_mut() {
            row.fill(0.0);
        }
        for id in self.living().collect::<Vec<_>>() {
            let species = &self.species[id as usize];
            let class = class_of(species);
            for cell in grid.cells() {
                self.contested[class][cell as usize] += species.suitability(
                    climate.temperature_c(cell),
                    climate.rain_mm(cell),
                    life.biome(cell),
                );
            }
        }
    }
}

/// Which of the four competing classes a species belongs to.
fn class_of(species: &Species) -> usize {
    (species.marine as usize) * 2 + (species.trophic == Trophic::Carnivore) as usize
}

/// A sample of the planet's temperatures, weighted by how productive each place is.
///
/// Drawn by repetition rather than by a proper weighted draw, because the sample is used
/// a few dozen times and building it this way keeps it a dozen lines.
fn warm_places(planet: &Lithosphere, life: &Biosphere, climate: &climate::Climate) -> Vec<f32> {
    let grid = planet.grid();
    let best = grid
        .cells()
        .map(|c| life.production(c))
        .fold(1.0f32, f32::max);
    let mut sample = Vec::new();
    for cell in grid.cells() {
        let weight = (life.production(cell) / best * 8.0).round() as usize;
        for _ in 0..weight {
            sample.push(climate.temperature_c(cell));
        }
    }
    if sample.is_empty() {
        sample.push(15.0);
    }
    sample
}

/// A name, from what the thing is rather than from a list.
///
/// Not taxonomy and not pretending to be. It exists so a species can be talked about at
/// all — "the large southern grazer" is a usable handle where "species 47" is not.
fn name_for(species: &Species, index: usize, rng: &mut Rng) -> String {
    const SIZES: [(f32, &str); 5] = [
        (0.5, "tiny"),
        (5.0, "small"),
        (50.0, "middling"),
        (300.0, "large"),
        (f32::MAX, "great"),
    ];
    let size = SIZES
        .iter()
        .find(|(under, _)| species.mass_kg < *under)
        .map(|(_, word)| *word)
        .unwrap_or("great");

    let habit = match (species.marine, species.trophic) {
        (true, Trophic::Herbivore) => ["grazer", "filterer", "drifter"],
        (true, Trophic::Carnivore) => ["hunter", "shark", "raider"],
        (false, Trophic::Herbivore) => ["browser", "grazer", "forager"],
        (false, Trophic::Carnivore) => ["stalker", "runner", "prowler"],
    };
    let habit = rng.pick(&habit).copied().unwrap_or("creature");
    let where_ = if species.coldest_c > 12.0 {
        "warm"
    } else if species.warmest_c < 6.0 {
        "cold"
    } else {
        "temperate"
    };
    format!("{size} {where_} {habit} {}", index + 1)
}

#[cfg(test)]
mod tests;
