//! People at the pace of continents.
//!
//! The last unfinished join. Below this the planet runs a megayear at a time; above it
//! people act in minutes and die in decades, and the two have never run together. The
//! design has said for a long time that connecting them "means deciding what a person *is*
//! when the clock is striding a megayear at a time", and the answer, once written down,
//! is short: **they are not anybody**.
//!
//! A megayear is thirty thousand human lifetimes. There is no individual at that
//! resolution, and pretending otherwise would be a lie told at a cost of a billion
//! simulated lives. What survives the projection upward is a *folk*: a number of people, in
//! a place, with a mean standing and a memory of where they came from. That is the same
//! move the level-of-detail machinery in §6 already makes between fine and coarse
//! neighbourhoods, taken one rung further.
//!
//! ## What deep time does to a people
//!
//! Everything, and none of it is scheduled. The planet is already moving on its own —
//! plates drift, mountains rise and wear down, the sea comes and goes, the thermostat
//! answers a brightening sun — and every one of those reaches a settlement through exactly
//! one channel: the habitability of the ground it stands on, recomputed each step from the
//! planet the people are actually on.
//!
//! So a coastal town whose shelf drains becomes an inland town. A temperate valley carried
//! into the subtropics becomes a desert and empties. Ground that was under ice at the start
//! is worth farming by the end. Nothing in this file knows about any of that; it asks
//! `settlement` how good each cell is now, and the answer moves because the planet did.
//!
//! ## What is not modelled
//!
//! Individuals, families, genetics, personality, and every one of the four behaviour
//! channels — all of which is the point, and all of which is recoverable. A deep-time run
//! records where and when people were, and a fine run can be founded on any of those
//! places at any of those moments. That is the backfill contract of §6: history that was
//! never simulated in detail can be simulated in detail later, because the aggregate it
//! was carried as is the aggregate a detailed run has to reproduce.
//!
//! Also not modelled: any technology whatsoever. These people farm what the land grows and
//! that is the whole of their economy, so their numbers track net primary production
//! directly. A folk that could irrigate, or fish, or trade, would decouple from it — and
//! that decoupling is the single most important thing about the actual human past.

use geo::CellId;
use sim_core::{Rng, WorldSeed};

use crate::Surface;

/// How many people a square kilometre of thoroughly good land carries.
///
/// Pre-agricultural to early-agrarian: foragers manage something like a tenth of a person
/// per square kilometre and early farmers forty, and a folk here is somewhere between. The
/// number matters less than that it is *fixed* — with no technology, a people's ceiling
/// moves only when their land does, which is the whole reason deep time is legible in
/// their numbers.
const FOLK_PER_KM2: f64 = 6.0;

/// How fast a population closes on what its land will carry, per megayear.
///
/// Effectively instantaneous. Humans double in decades and a megayear is thirty thousand
/// generations, so at this resolution a population is *always* at its ceiling and the only
/// question is where the ceiling is. Anything less than total convergence here would be
/// modelling a transient far shorter than the step.
const SETTLES_TO_CEILING: f64 = 0.98;

/// Habitability below which a place is abandoned.
///
/// Not zero. A settlement fails long before its ground becomes literally uninhabitable —
/// the last people leave when it stops being worth staying, and that is a threshold rather
/// than a limit.
const ABANDON_BELOW: f32 = 0.06;
/// Habitability above which unoccupied ground is worth settling.
///
/// Higher than the abandonment threshold, and the gap between them is deliberate. Without
/// it a cell hovering at the line is founded and abandoned on alternate steps forever,
/// which is not history, it is a rounding error with a name.
const SETTLE_ABOVE: f32 = 0.22;
/// How far apart new settlements must be, in rings of the grid.
const APART: usize = 1;
/// The most settlements a world will carry at once.
///
/// A bound on the bookkeeping rather than a claim about the world. It binds only on a
/// planet almost all of which is worth living on.
const MOST_SETTLEMENTS: usize = 64;

/// A people, in a place, at aggregate resolution.
#[derive(Clone, Debug)]
pub struct Folk {
    pub name: String,
    pub cell: CellId,
    /// How many of them there are.
    pub souls: u64,
    /// When they arrived here, in megayears from the start of the run.
    pub founded_myr: f64,
    /// The folk they split from, if they were not there at the beginning.
    pub parent: Option<usize>,
    /// The best their ground has been since they came, and the worst.
    pub best_ground: f32,
    pub worst_ground: f32,
}

/// Something that happened to a people, at the scale where it is worth recording.
#[derive(Clone, Debug, PartialEq)]
pub enum Epoch {
    /// A people took ground nobody was on.
    Settled { folk: usize, cell: CellId },
    /// Their ground stopped being worth standing on.
    Abandoned {
        folk: usize,
        cell: CellId,
        why: Ruin,
    },
    /// Everyone, everywhere, is gone.
    Extinct,
}

/// Why a place stopped being one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ruin {
    /// The sea took it.
    Drowned,
    /// Ice took it.
    Frozen,
    /// It dried out.
    Parched,
    /// It rose, or the land around it did.
    Uplifted,
    /// The ground is still there and will no longer feed anybody.
    Barren,
}

impl Ruin {
    pub const fn label(self) -> &'static str {
        match self {
            Ruin::Drowned => "drowned",
            Ruin::Frozen => "frozen",
            Ruin::Parched => "parched",
            Ruin::Uplifted => "thrown up",
            Ruin::Barren => "worn out",
        }
    }
}

/// One reading of a populated world, at one moment in deep time.
#[derive(Clone, Debug)]
pub struct Age {
    pub myr: f64,
    pub souls: u64,
    pub settlements: usize,
    /// The share of the surface anybody could live on.
    pub habitable: f32,
    pub mean_temperature_c: f32,
    pub land_fraction: f32,
    /// Settlements founded and lost since the previous reading.
    pub founded: usize,
    pub lost: usize,
}

/// A populated world, running at the pace of its planet.
pub struct Ages {
    pub surface: Surface,
    pub folk: Vec<Folk>,
    pub history: Vec<Epoch>,
    pub readings: Vec<Age>,
    seed: WorldSeed,
    myr: f64,
    /// Every settlement ever founded, including the ones that failed.
    pub ever: usize,
    pub lost: usize,
}

impl Ages {
    /// Put people on a planet and let the planet get on with it.
    pub fn begin(seed: WorldSeed, surface: Surface) -> Ages {
        let mut ages = Ages {
            surface,
            folk: Vec::new(),
            history: Vec::new(),
            readings: Vec::new(),
            seed,
            myr: 0.0,
            ever: 0,
            lost: 0,
        };
        let mut rng = seed.stream(sim_core::Domain::Naming, 7, 0);
        ages.settle_what_is_empty(&mut rng);
        ages.read();
        ages
    }

    pub fn myr(&self) -> f64 {
        self.myr
    }

    /// Everybody, everywhere.
    pub fn souls(&self) -> u64 {
        self.folk.iter().map(|f| f.souls).sum()
    }

    /// Advance the planet, and let it do what it does to the people on it.
    ///
    /// The order matters and is the whole design. The planet moves *first* and with no
    /// knowledge that anybody is on it; then the people are told what their ground is now.
    /// Causation runs one way, which is what makes this deep time with people in it rather
    /// than a world that arranges itself around them.
    pub fn step_myr(&mut self, dt: f32, rng: &mut Rng) {
        self.surface.step_myr(dt, rng);
        self.myr += dt as f64;

        let before = self.folk.len();
        let lost = self.abandon_what_failed();
        self.settle_what_is_empty(rng);
        let founded = self.folk.len() + lost - before;

        self.grow();
        self.read_with(founded, lost);

        if self.folk.is_empty() && !matches!(self.history.last(), Some(Epoch::Extinct)) {
            self.history.push(Epoch::Extinct);
        }
    }

    /// Run for a span, in steps no longer than the planet can take in one.
    pub fn run_myr(&mut self, span: f64, step: f32, rng: &mut Rng) {
        let mut done = 0.0;
        while done < span {
            let dt = step.min((span - done) as f32);
            self.step_myr(dt, rng);
            done += dt as f64;
        }
    }

    /// The habitability of every cell, as it is now.
    pub fn habitability(&self) -> settlement::Habitability {
        settlement::Habitability::of(&self.surface.planet, &self.surface.climate, &self.surface.life)
    }

    /// Let go of ground that stopped being worth standing on.
    fn abandon_what_failed(&mut self) -> usize {
        let habitability = self.habitability();
        let planet = &self.surface.planet;
        let climate = &self.surface.climate;

        let mut lost = Vec::new();
        for (index, folk) in self.folk.iter_mut().enumerate() {
            let cell = folk.cell;
            let score = habitability.score(cell);
            folk.best_ground = folk.best_ground.max(score);
            folk.worst_ground = folk.worst_ground.min(score);
            if score >= ABANDON_BELOW {
                continue;
            }
            // Why it failed, which is the part worth recording. The planet knows; it is
            // only a matter of asking it the right question in the right order — the sea
            // beats the ice beats the drought, because a drowned cell is also cold and
            // also has no rain and drowning is what happened to it.
            let why = if !planet.is_land(cell) {
                Ruin::Drowned
            } else if climate.is_frozen(cell) {
                Ruin::Frozen
            } else if climate.rain_mm(cell) < 150.0 {
                Ruin::Parched
            } else if planet.height_above_sea_m(cell) > 3000.0 {
                Ruin::Uplifted
            } else {
                Ruin::Barren
            };
            lost.push((index, cell, why));
        }

        for (index, cell, why) in lost.iter().rev() {
            self.history.push(Epoch::Abandoned {
                folk: *index,
                cell: *cell,
                why: *why,
            });
            self.folk.remove(*index);
            self.lost += 1;
        }
        lost.len()
    }

    /// Take ground that has become worth taking.
    ///
    /// Newly habitable cells are settled by whoever is nearest, which is why the record
    /// keeps a parent: a people spreading into a thawing continent is one people, and a
    /// people appearing on the far side of an ocean is another.
    fn settle_what_is_empty(&mut self, rng: &mut Rng) {
        if self.folk.len() >= MOST_SETTLEMENTS {
            return;
        }
        let habitability = self.habitability();
        let planet = &self.surface.planet;
        let grid = planet.grid();

        let mut candidates: Vec<(CellId, f32)> = grid
            .cells()
            .filter(|&c| habitability.score(c) >= SETTLE_ABOVE)
            .map(|c| (c, habitability.score(c)))
            .collect();
        candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));

        for (cell, _) in candidates {
            if self.folk.len() >= MOST_SETTLEMENTS {
                break;
            }
            if self
                .folk
                .iter()
                .any(|f| f.cell == cell || rings_apart(grid, f.cell, cell, APART))
            {
                continue;
            }
            // Whoever is nearest, if anybody is near enough to have walked it.
            let parent = self
                .folk
                .iter()
                .enumerate()
                .filter(|(_, f)| {
                    grid.distance_km(f.cell, cell, geo::EARTH_RADIUS_KM) < WALKED_KM
                })
                .min_by(|(_, a), (_, b)| {
                    grid.distance_km(a.cell, cell, geo::EARTH_RADIUS_KM)
                        .total_cmp(&grid.distance_km(b.cell, cell, geo::EARTH_RADIUS_KM))
                })
                .map(|(i, _)| i);

            let terrain = society::Terrain {
                cell,
                latitude: grid.position(cell).latitude().to_degrees() as f32,
                longitude: grid.position(cell).longitude().to_degrees() as f32,
                elevation_m: planet.height_above_sea_m(cell),
                fertility: habitability.fertility(cell),
                reach: habitability.reach(cell),
                harshness: habitability.harshness(cell),
                carrying: 1,
                biome: self.surface.life.biome(cell).label(),
            };
            let coastal = grid.neighbours(cell).iter().any(|&n| !planet.is_land(n));
            let name = settlement::naming::name_for(&terrain, coastal, rng);

            self.folk.push(Folk {
                name,
                cell,
                // One family walks in. What happens next is the land's business.
                souls: 40,
                founded_myr: self.myr,
                parent,
                best_ground: habitability.score(cell),
                worst_ground: habitability.score(cell),
            });
            self.ever += 1;
            self.history.push(Epoch::Settled {
                folk: self.folk.len() - 1,
                cell,
            });
        }
    }

    /// Let every people find the number its land will carry.
    fn grow(&mut self) {
        let planet = &self.surface.planet;
        let grid = planet.grid();
        let life = &self.surface.life;
        let habitability = self.habitability();

        for folk in &mut self.folk {
            let area = grid.area_km2(folk.cell, geo::EARTH_RADIUS_KM);
            // What the ground grows, against what good farmland grows. Production is the
            // only economy these people have, so their ceiling is it.
            let fed = (life.production(folk.cell) / 1200.0).clamp(0.0, 1.6) as f64;
            let livable = habitability.score(folk.cell).clamp(0.0, 1.0) as f64;
            let ceiling = (area * FOLK_PER_KM2 * fed * livable).max(0.0);
            let now = folk.souls as f64;
            folk.souls = (now + (ceiling - now) * SETTLES_TO_CEILING).max(0.0).round() as u64;
        }
    }

    fn read(&mut self) {
        self.read_with(self.folk.len(), 0);
    }

    fn read_with(&mut self, founded: usize, lost: usize) {
        let habitability = self.habitability();
        self.readings.push(Age {
            myr: self.myr,
            souls: self.souls(),
            settlements: self.folk.len(),
            habitable: habitability.habitable_fraction(&self.surface.planet),
            mean_temperature_c: self.surface.climate.mean_temperature_c(&self.surface.planet),
            land_fraction: self.surface.planet.land_fraction(),
            founded,
            lost,
        });
    }

    pub fn seed(&self) -> WorldSeed {
        self.seed
    }
}

/// How far a people will walk to take new ground, in kilometres.
///
/// Generous, because a megayear is long enough to walk anywhere that can be walked to. What
/// it actually rules out is crossing water, which is the distinction that matters: a
/// continent is settled by the people already on it, and an island is settled by nobody.
const WALKED_KM: f64 = 4_000.0;

/// Whether two cells are within `rings` steps of each other.
fn rings_apart(grid: &geo::Grid, a: CellId, b: CellId, rings: usize) -> bool {
    if a == b {
        return true;
    }
    let mut seen = vec![a];
    let mut frontier = vec![a];
    for _ in 0..rings {
        let mut next = Vec::new();
        for cell in frontier.drain(..) {
            for &n in grid.neighbours(cell) {
                if n == b {
                    return true;
                }
                if !seen.contains(&n) {
                    seen.push(n);
                    next.push(n);
                }
            }
        }
        frontier = next;
    }
    false
}


