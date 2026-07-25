//! How a fauna becomes a different fauna.
//!
//! Phase 7 gave the planet animals that live where they can live and die when they
//! cannot. What it could not give it was *turnover*: nothing new ever arose, so a run
//! long enough to matter thinned steadily towards an empty world. This is the other half.
//!
//! Two mechanisms, and between them they are most of what macroevolution is over deep
//! time.
//!
//! **Adaptation.** A species' tolerances drift towards the conditions it is actually
//! living in. Not because anything chooses to — the population in the warm end of a range
//! contributes more offspring than the population in the cold end simply by being larger,
//! and the band follows. It is slow, it is bounded by how fast a lineage can track a
//! moving target, and when the climate moves faster than that the species does not adapt,
//! it dies. Which of those happens is the whole question, and it is not decided here.
//!
//! **Allopatric speciation.** A range broken into pieces that cannot reach each other
//! stops being one population. Left apart long enough, the pieces diverge past the point
//! of being one species, and what was one lineage is two. This is the dominant mode of
//! speciation on the real planet and it needs no mechanism of its own: it is what
//! geography does to a range, and the geography here is already moving. Continents rift,
//! seas open, mountains rise, ice sheets advance — and each of those cuts ranges in half.
//!
//! ## What is not here
//!
//! Sympatric speciation, sexual selection, character displacement, coevolution, and
//! anything at all about the genome. A species' traits are a handful of numbers and they
//! move by a rule rather than by inheritance from individuals — the polygenic machinery in
//! `genetics` exists and is used for people, but wiring it to demes would mean giving
//! every deme a founder genome and a breeding population, which is a different simulation
//! at a different cost.

use std::collections::BTreeMap;

use biome::Biosphere;
use climate::Climate;
use ecology::{Ecology, Species, SpeciesId};
use geo::{CellId, Lithosphere};
use sim_core::Rng;

/// How far a species' temperature band can shift in a megayear, in °C.
///
/// The speed limit on evolutionary tracking, and it is the number that decides whether a
/// climate shift is survivable or fatal. Measured rates of thermal-niche evolution in
/// vertebrates run well under a degree per million years — which is why the usual outcome
/// of rapid warming is not adaptation but a range shift, and failing that, extinction.
const TRACKING_C_PER_MYR: f32 = 0.35;

/// How far apart two fragments of a range must stay, in megayears, before they are two
/// species.
///
/// A few million years is about right for vertebrates: long enough that the Isthmus of
/// Panama's closure is still producing separations rather than separate species, short
/// enough that an ice age can do it.
const ISOLATION_MYR: f32 = 10.0;
/// The smallest fragment worth calling a population rather than a straggler.
///
/// Generous, and deliberately. A planet's geography leaves ranges dotted with islands and
/// mountain valleys, and treating every one of them as a founding population turns
/// speciation into a fountain: the first version split every species several times over
/// within a hundred megayears and ran straight into the ceiling.
const FOUNDER_CELLS: usize = 10;
/// How much a daughter species differs from its parent at birth, in °C of tolerance.
const DIVERGENCE_C: f64 = 2.5;
/// How many species one cell of the planet is worth, as a rough count of niches.
///
/// The planet's diversity ceiling, in other words, and it is a real quantity rather than
/// a backstop: a bigger and more varied surface holds more kinds of animal, which is the
/// species-area relationship stated the other way round.
const NICHES_PER_CELL: f32 = 0.05;

/// A record of who came from whom.
#[derive(Clone, Copy, Debug)]
pub struct Lineage {
    pub parent: Option<SpeciesId>,
    pub arose_myr: f64,
}

/// The evolving part of an ecology: what descends from what, and how long fragments of a
/// range have been apart.
pub struct Evolution {
    lineage: Vec<Lineage>,
    /// How long each species has been split, in megayears, keyed by species.
    apart: BTreeMap<SpeciesId, f32>,
    age_myr: f64,
    pub speciations: usize,
}

impl Evolution {
    /// Begin, with every species that already exists as its own root.
    pub fn beginning(ecology: &Ecology) -> Evolution {
        Evolution {
            lineage: (0..ecology.species().len())
                .map(|_| Lineage {
                    parent: None,
                    arose_myr: 0.0,
                })
                .collect(),
            apart: BTreeMap::new(),
            age_myr: 0.0,
            speciations: 0,
        }
    }

    /// Who this species descends from, if anyone.
    pub fn parent_of(&self, id: SpeciesId) -> Option<SpeciesId> {
        self.lineage.get(id as usize).and_then(|l| l.parent)
    }

    pub fn arose(&self, id: SpeciesId) -> f64 {
        self.lineage.get(id as usize).map_or(0.0, |l| l.arose_myr)
    }

    /// The chain of ancestors of a species, nearest first.
    pub fn ancestry(&self, id: SpeciesId) -> Vec<SpeciesId> {
        let mut chain = Vec::new();
        let mut at = self.parent_of(id);
        // Bounded because a lineage cannot be longer than the number of species, and a
        // cycle here would hang the run rather than merely being wrong.
        while let Some(parent) = at {
            if chain.contains(&parent) {
                break;
            }
            chain.push(parent);
            at = self.parent_of(parent);
        }
        chain
    }

    /// How deep in the tree a species sits — how many originations separate it from a
    /// founding lineage.
    pub fn depth(&self, id: SpeciesId) -> usize {
        self.ancestry(id).len()
    }

    /// Everything descended from a species, at any depth.
    pub fn descendants(&self, id: SpeciesId) -> Vec<SpeciesId> {
        (0..self.lineage.len() as SpeciesId)
            .filter(|other| *other != id && self.ancestry(*other).contains(&id))
            .collect()
    }

    /// Let a megayear of evolution happen.
    pub fn step_myr(
        &mut self,
        planet: &Lithosphere,
        life: &Biosphere,
        climate: &Climate,
        ecology: &mut Ecology,
        dt: f32,
        rng: &mut Rng,
    ) {
        self.age_myr += dt as f64;
        // Species added since last time — by extinction's counterpart or by anything else
        // that introduced one — get their own root.
        while self.lineage.len() < ecology.species().len() {
            self.lineage.push(Lineage {
                parent: None,
                arose_myr: self.age_myr,
            });
        }

        self.adapt(planet, life, climate, ecology, dt);
        self.split(planet, life, climate, ecology, dt, rng);
    }

    /// Move every species' tolerances towards the conditions it is actually living in.
    fn adapt(
        &mut self,
        planet: &Lithosphere,
        life: &Biosphere,
        climate: &Climate,
        ecology: &mut Ecology,
        dt: f32,
    ) {
        let grid = planet.grid();
        let _ = life;
        for id in ecology.living().collect::<Vec<_>>() {
            // Where the animals actually are, weighted by how many of them are there.
            // A range's cold edge holds few and its middle holds many, so the mean the
            // band chases is the mean of the population and not of the map.
            let mut weight = 0.0f64;
            let mut warmth = 0.0f64;
            for cell in grid.cells() {
                let held = ecology.biomass_of(id, cell) as f64;
                if held <= 0.0 {
                    continue;
                }
                weight += held;
                warmth += held * climate.temperature_c(cell) as f64;
            }
            if weight <= 0.0 {
                continue;
            }

            let lived_at = (warmth / weight) as f32;
            let species = ecology.get(id);
            let centre = (species.coldest_c + species.warmest_c) / 2.0;
            // Bounded, and the bound is the point: a lineage can only track so fast, and
            // a climate moving faster than this is one a species does not adapt to.
            let shift =
                (lived_at - centre).clamp(-TRACKING_C_PER_MYR * dt, TRACKING_C_PER_MYR * dt);
            ecology.shift_tolerance(id, shift);
        }
    }

    /// Break species whose ranges have been in pieces for long enough.
    fn split(
        &mut self,
        planet: &Lithosphere,
        life: &Biosphere,
        climate: &Climate,
        ecology: &mut Ecology,
        dt: f32,
        rng: &mut Rng,
    ) {
        // Diversity-dependent diversification, which is the standard way this is
        // modelled and the thing that was missing: the fuller the world, the harder it is
        // for a new lineage to establish, because there is less unoccupied opportunity to
        // establish *into*. Without it origination is a fountain — every split succeeds,
        // the count runs straight to whatever cap exists, and sits there.
        let ceiling = planet.grid().len() as f32 * NICHES_PER_CELL;
        let room = (1.0 - ecology.richness() as f32 / ceiling).clamp(0.0, 1.0);
        if room <= 0.0 {
            return;
        }

        for id in ecology.living().collect::<Vec<_>>() {
            let pieces = fragments(planet, ecology, id);
            if pieces.len() < 2 {
                self.apart.remove(&id);
                continue;
            }
            let held = self.apart.entry(id).or_insert(0.0);
            *held += dt;
            if *held < ISOLATION_MYR {
                continue;
            }
            self.apart.remove(&id);

            // The largest piece keeps the name. Every other piece big enough to be a
            // population of its own becomes a species, diverged a little from where it
            // started — which is what being apart does.
            let mut ordered = pieces;
            ordered.sort_by_key(|piece| std::cmp::Reverse(piece.len()));
            for piece in ordered.into_iter().skip(1) {
                if piece.len() < FOUNDER_CELLS || !rng.chance(room as f64) {
                    continue;
                }
                let parent = ecology.get(id).clone();
                let drift = rng.range_f64(-DIVERGENCE_C, DIVERGENCE_C) as f32;
                let daughter = Species {
                    name: format!("{} (daughter)", parent.name),
                    coldest_c: parent.coldest_c + drift,
                    warmest_c: parent.warmest_c + drift,
                    arose_myr: self.age_myr,
                    ..parent
                };
                let child = ecology.split_off(&piece, id, daughter, life, climate);
                while self.lineage.len() <= child as usize {
                    self.lineage.push(Lineage {
                        parent: None,
                        arose_myr: self.age_myr,
                    });
                }
                self.lineage[child as usize] = Lineage {
                    parent: Some(id),
                    arose_myr: self.age_myr,
                };
                self.speciations += 1;
            }
        }
    }
}

/// The connected pieces of a species' range.
///
/// Connected across the cell graph, which is what "can reach each other" means on this
/// planet: a population on the far side of an ocean or a mountain range is a population
/// that no longer breeds with the one it came from, and that is the whole of allopatry.
fn fragments(planet: &Lithosphere, ecology: &Ecology, id: SpeciesId) -> Vec<Vec<CellId>> {
    let grid = planet.grid();
    let present = |cell: CellId| ecology.biomass_of(id, cell) > ecology.presence_floor(cell);

    let mut seen = vec![false; grid.len()];
    let mut pieces = Vec::new();
    let mut stack = Vec::new();
    for start in grid.cells() {
        if seen[start as usize] || !present(start) {
            continue;
        }
        let mut piece = Vec::new();
        seen[start as usize] = true;
        stack.push(start);
        while let Some(cell) = stack.pop() {
            piece.push(cell);
            for &n in grid.neighbours(cell) {
                if !seen[n as usize] && present(n) {
                    seen[n as usize] = true;
                    stack.push(n);
                }
            }
        }
        pieces.push(piece);
    }
    pieces
}

#[cfg(test)]
mod tests;
