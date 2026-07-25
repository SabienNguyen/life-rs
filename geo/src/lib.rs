//! The solid planet: a geodesic grid, plates that turn on it, crust that floats, and
//! rivers that take it apart again.
//!
//! Nothing here is a map. There is no authored coastline, no drawn mountain range, no
//! scripted supercontinent. There is a sphere, a dozen rigid caps rotating about
//! randomly drawn Euler poles, and four rules for what happens where two of them meet.
//! Continents collide because they drift; they aggregate because collision welds the
//! plates carrying them into one; that aggregate eventually rifts because a plate large
//! enough to sit over its own insulated mantle is a plate under tension. The
//! supercontinent cycle is not in the code. It is what the code does.
//!
//! ## How motion is represented
//!
//! The obvious approach — hold crust in the grid and shuffle it between cells each step
//! — does not work. A plate moves fifty kilometres in a megayear and the cells are a
//! hundred and twelve across, so every step rounds to no motion at all and the planet
//! is frozen. Instead each parcel of crust remembers **where it sits within its own
//! plate**, and its position on the planet is that coordinate turned by however far the
//! plate has rotated so far. Sub-cell motion accumulates exactly; the grid is only ever
//! consulted at the end, to ask which cell a parcel has arrived in.
//!
//! Resolving those arrivals is the whole of tectonics:
//!
//! - two parcels want the same cell → they are converging, and one of them loses;
//! - no parcel wants a cell → the plates have pulled apart and ocean floor is born there.
//!
//! Because every cell ends each step holding exactly one parcel, the state stays a set
//! of flat per-cell arrays with no allocation, no free lists, and no drift in the count.
//!
//! ## Where the rain comes in
//!
//! Erosion needs to know how hard it is raining, and this crate has no idea — it sits
//! below the climate and always will. So [`Lithosphere::set_runoff`] is the one thing
//! anything outside reaches in to set, and a caller that has a climate is expected to
//! keep it current. A planet with no climate is rained on evenly, which is what the first
//! version of this did everywhere and is a worse error than it sounds: a desert then
//! wears down as fast as a rainforest, the continents plane away, and the carbon
//! thermostat downstream loses the rock it needs in order to regulate anything.

pub mod crust;
pub mod erosion;
pub mod grid;
pub mod plates;
pub mod sphere;

use std::collections::{BTreeMap, BTreeSet};

use sim_core::Rng;

pub use crust::CrustType;
pub use grid::{CellId, Grid};
pub use plates::{Plate, PlateId};
pub use sphere::Vec3;

use crust::OCEANIC_THICKNESS_KM;
use erosion::Erosion;

/// Earth's radius, in kilometres.
pub const EARTH_RADIUS_KM: f64 = 6371.0;
/// Earth's surface water, in cubic kilometres.
pub const EARTH_WATER_KM3: f64 = 1.335e9;

/// What an undisturbed continental interior is worth, in kilometres.
const CRATON_THICKNESS_KM: f32 = 39.0;
/// How much thinner a coast is than the interior behind it.
const MARGIN_TAPER_KM: f32 = 18.0;
/// Over how many cells the thinning happens.
const MARGIN_HOPS: f32 = 3.0;
/// No sea floor is older than this, because by then it has been subducted.
const OLDEST_SEAFLOOR_MYR: f32 = 150.0;

/// Crust cannot pile up forever: past this the root founders and delaminates, which is
/// why no plateau on the real planet is much over five kilometres.
const MAX_CRUST_KM: f32 = 70.0;
/// What a volcanic arc adds to the crust above it, in kilometres per megayear.
///
/// Calibrated against the measured figure rather than chosen: continental crust grows by
/// something like two cubic kilometres a year, which spread over the share of a
/// level-three sphere that sits on a convergent boundary is about five hundredths of a
/// kilometre of thickening per megayear. It is a small number and it is the only thing
/// standing between the planet and a slow death — erosion and collision both take
/// continental crust away, and without magmatism putting it back a planet planes itself
/// down to a waterworld inside a couple of billion years.
const ARC_GROWTH_KM_PER_MYR: f32 = 0.05;
/// Thickened island arc stops being ocean floor and becomes new continent. This is how
/// continental crust grows, and roughly how much of it did.
const ARC_MATURES_AT_KM: f32 = 20.0;

/// Crust thicker than this cannot hold itself up, and spreads sideways.
///
/// Gravitational collapse, and it is happening in Tibet now. Without it collisions are a
/// one-way street — every continental cell that loses a collision becomes thickness in
/// its neighbour, so continental *area* only ever shrinks, and a planet run for half a
/// billion years ends up with a few impossibly thick stubs and an ocean. Collapse is the
/// return path: thickness turns back into area, and the continents keep their extent.
const COLLAPSE_AT_KM: f32 = 50.0;
/// The share of the excess that flows out per megayear.
const COLLAPSE_RATE: f32 = 0.05;

/// Expected megayears between one plate changing its pole and rate.
const REORGANISATION_INTERVAL_MYR: f64 = 80.0;
/// A plate carrying more than this share of the planet's continental crust is sitting
/// on its own heat, and is a candidate to rift.
const RIFT_THRESHOLD_SHARE: f32 = 0.28;
/// Expected megayears to rifting once a plate is that big.
const RIFT_INTERVAL_MYR: f64 = 60.0;
/// How much collision two plates must accumulate before they are one plate.
///
/// Set by plate *count* rather than by any measurement of welding: weld too readily and
/// the planet ends up with three plates, which means very little ridge, which means the
/// sea floor stops being recycled and the whole conveyor quietly stops. Earth runs about
/// fifteen.
const WELD_AT: f32 = 45.0;
/// The most plates the planet will carry. A backstop on runaway rifting, not physics.
const MAX_PLATES: usize = 26;

/// No cell, no parcel.
const NOWHERE: CellId = CellId::MAX;
/// How far from a cell's centre a parcel may be and still be said to cover it, as a
/// multiple of the cell width. A cell that can find nothing this close is a cell the
/// plates have pulled apart, and it gets new ocean floor.
const REACH: f64 = 0.85;
/// How far a cell will reach for leftover crust once its neighbours have chosen, again
/// as a multiple of the cell width. Wider than [`REACH`], because by this point the
/// question is no longer "what covers me" but "is there any crust here at all".
const STRETCH: f64 = 2.0;

/// How near a second plate's parcel has to get, relative to the first's, before the two
/// are treated as contesting the cell rather than one plainly holding it.
const CONTEST: f64 = 1.30;

/// What is happening where two plates meet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Boundary {
    Interior,
    Divergent,
    Convergent,
    Transform,
}

impl Boundary {
    pub const fn label(self) -> &'static str {
        match self {
            Boundary::Interior => "interior",
            Boundary::Divergent => "divergent",
            Boundary::Convergent => "convergent",
            Boundary::Transform => "transform",
        }
    }
}

/// The solid planet.
pub struct Lithosphere {
    grid: Grid,
    radius_km: f64,
    /// How wide a cell is, in radians. Measured once — the grid never changes.
    cell_radians: f64,
    water_km3: f64,
    age_myr: f64,

    plates: Vec<Plate>,

    // ---- per cell, in present position -------------------------------------------
    plate_of: Vec<PlateId>,
    /// Where this parcel sits within its own plate.
    frame: Vec<Vec3>,
    crust: Vec<CrustType>,
    thickness_km: Vec<f32>,
    crust_age_myr: Vec<f32>,
    sediment_m: Vec<f32>,
    elevation_m: Vec<f32>,
    boundary: Vec<Boundary>,
    area_km2: Vec<f64>,

    sea_level_m: f32,
    /// Rainfall relative to the reference planet, if a climate has told us any.
    runoff: Option<Vec<f32>>,

    // ---- scratch, held to keep a megayear allocation-free --------------------------
    erosion: Erosion,
    stripped_m: Vec<f32>,
    deposited_m: Vec<f32>,
    /// Where each parcel has got to, this step.
    present: Vec<Vec3>,
    /// Which parcel each cell reached for, and how far away it was.
    wanted: Vec<CellId>,
    want_gap: Vec<f64>,
    /// Which cell ended up holding each parcel.
    claimed: Vec<CellId>,
    /// Cells sorted by how near their best parcel came.
    order: Vec<CellId>,
    /// Each cell's own index plus two rings of neighbours, flattened. Built once.
    near: Vec<CellId>,
    near_start: Vec<u32>,
    next_plate: Vec<PlateId>,
    next_frame: Vec<Vec3>,
    next_crust: Vec<CrustType>,
    next_thickness: Vec<f32>,
    next_age: Vec<f32>,
    next_sediment: Vec<f32>,
    /// Accumulated continent-on-continent collision, in megayears, per plate pair.
    pressure: BTreeMap<(PlateId, PlateId), f32>,
    /// Which pairs collided during the step being resolved.
    colliding: BTreeSet<(PlateId, PlateId)>,
    churn: Churn,
}

/// What the last step's rounding did — a diagnostic, not part of the physics.
///
/// Matching a rotated set of parcels onto a fixed grid is never quite a bijection: a
/// few cells find nothing and a few parcels find nowhere. Watching these numbers is how
/// three separate crust leaks in this module were found — one of them converting four
/// percent of the planet to sea floor every step — so the counters stay in.
#[derive(Clone, Copy, Debug, Default)]
pub struct Churn {
    /// Cells that genuinely rifted open and were given new ocean floor.
    pub opened: usize,
    /// Cells left empty by rounding rather than by geology, filled from their
    /// neighbours. A number that climbs with the length of the step.
    pub patched: usize,
    /// Parcels no cell had room for. At a trench that is subduction; anywhere else it
    /// is the same rounding seen from the other side.
    pub stranded: usize,
}

impl Lithosphere {
    /// A new planet: plates drawn at random, continents grown from random cratons.
    ///
    /// `land_fraction` is the *initial* share of the surface that is continental crust.
    /// It does not stay put — arcs mature into continent, collisions weld it into
    /// smaller and thicker masses, and the sea moves independently of all of it.
    pub fn genesis(
        level: u8,
        plate_count: usize,
        land_fraction: f32,
        rng: &mut Rng,
    ) -> Lithosphere {
        let grid = Grid::new(level);
        let n = grid.len();

        let mut plates: Vec<Plate> = (0..plate_count.max(2))
            .map(|_| Plate::random(rng))
            .collect();
        // Drawn independently, so the first plate may be assigned no cells at all by the
        // flood fill; that is fine and it will simply be inactive.
        let plate_of = flood_fill_plates(&grid, plates.len(), rng);
        for (id, plate) in plates.iter_mut().enumerate() {
            plate.active = plate_of.contains(&(id as PlateId));
        }

        let continental = grow_continents(&grid, land_fraction, rng);

        // Two smooth fields, both measured in hops across the cell graph.
        //
        // How far from dry land: a continental margin was stretched when the continent
        // rifted, so it thins towards the sea over several hundred kilometres rather
        // than ending at a cliff. Tapering only the outermost ring of cells gives a
        // continent that is essentially all dry, and a planet with no shelf seas at all.
        //
        // And how far from a continent, for the ocean floor: sea floor is born at a
        // ridge and grows older as it travels away, so age is a smooth ramp, not a
        // number drawn afresh for every cell. Drawing it per cell put five kilometres of
        // relief between neighbouring points of abyssal plain — which is invisible in
        // any summary statistic and unmissable the moment the planet is drawn.
        let from_sea = hops_from(&grid, |c| !continental[c as usize]);
        let from_land = hops_from(&grid, |c| continental[c as usize]);
        let deepest = from_land.iter().copied().max().unwrap_or(1).max(1) as f32;

        let mut crust = Vec::with_capacity(n);
        let mut thickness_km = Vec::with_capacity(n);
        let mut crust_age_myr = Vec::with_capacity(n);
        for cell in 0..n {
            if continental[cell] {
                crust.push(CrustType::Continental);
                let inland = (from_sea[cell] as f32 / MARGIN_HOPS).min(1.0);
                let taper = MARGIN_TAPER_KM * (1.0 - inland);
                // And a little variation, so the first coastline is not a contour line.
                thickness_km.push(CRATON_THICKNESS_KM - taper + rng.range_f64(-1.5, 1.5) as f32);
                crust_age_myr.push(rng.range_f64(200.0, 2000.0) as f32);
            } else {
                crust.push(CrustType::Oceanic);
                thickness_km.push(OCEANIC_THICKNESS_KM);
                // Oldest against the continents it rifted from, youngest in mid-ocean
                // where the ridge is. Capped where the real sea floor is capped: past
                // about a hundred and fifty megayears it has all been subducted.
                let out = from_land[cell] as f32 / deepest;
                crust_age_myr.push(OLDEST_SEAFLOOR_MYR * (1.0 - out).clamp(0.0, 1.0));
            }
        }

        let (near, near_start) = two_rings(&grid);
        let area_km2: Vec<f64> = grid
            .cells()
            .map(|c| grid.area_km2(c, EARTH_RADIUS_KM))
            .collect();
        let frame: Vec<Vec3> = grid.cells().map(|c| grid.position(c)).collect();

        let mut planet = Lithosphere {
            radius_km: EARTH_RADIUS_KM,
            cell_radians: grid.spacing_km(EARTH_RADIUS_KM) / EARTH_RADIUS_KM,
            water_km3: EARTH_WATER_KM3,
            age_myr: 0.0,
            plates,
            plate_of,
            frame,
            crust,
            thickness_km,
            crust_age_myr,
            sediment_m: vec![0.0; n],
            elevation_m: vec![0.0; n],
            boundary: vec![Boundary::Interior; n],
            area_km2,
            sea_level_m: 0.0,
            runoff: None,
            erosion: Erosion::new(n),
            stripped_m: vec![0.0; n],
            deposited_m: vec![0.0; n],
            present: vec![Vec3::new(0.0, 0.0, 1.0); n],
            wanted: vec![NOWHERE; n],
            want_gap: vec![0.0; n],
            claimed: vec![NOWHERE; n],
            order: Vec::with_capacity(n),
            near,
            near_start,
            next_plate: vec![0; n],
            next_frame: vec![Vec3::new(0.0, 0.0, 1.0); n],
            next_crust: vec![CrustType::Oceanic; n],
            next_thickness: vec![0.0; n],
            next_age: vec![0.0; n],
            next_sediment: vec![0.0; n],
            pressure: BTreeMap::new(),
            colliding: BTreeSet::new(),
            churn: Churn::default(),
            grid,
        };
        planet.settle();
        planet
    }

    // ---- reading the planet ----------------------------------------------------

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    pub fn age_myr(&self) -> f64 {
        self.age_myr
    }

    pub fn sea_level_m(&self) -> f32 {
        self.sea_level_m
    }

    pub fn elevation_m(&self, cell: CellId) -> f32 {
        self.elevation_m[cell as usize]
    }

    /// Height above the sea, which is not the same as height above the datum once the
    /// sea has moved.
    pub fn height_above_sea_m(&self, cell: CellId) -> f32 {
        self.elevation_m[cell as usize] - self.sea_level_m
    }

    pub fn is_land(&self, cell: CellId) -> bool {
        self.elevation_m[cell as usize] > self.sea_level_m
    }

    pub fn crust(&self, cell: CellId) -> CrustType {
        self.crust[cell as usize]
    }

    pub fn thickness_km(&self, cell: CellId) -> f32 {
        self.thickness_km[cell as usize]
    }

    pub fn crust_age_myr(&self, cell: CellId) -> f32 {
        self.crust_age_myr[cell as usize]
    }

    pub fn sediment_m(&self, cell: CellId) -> f32 {
        self.sediment_m[cell as usize]
    }

    pub fn plate_of(&self, cell: CellId) -> PlateId {
        self.plate_of[cell as usize]
    }

    pub fn boundary(&self, cell: CellId) -> Boundary {
        self.boundary[cell as usize]
    }

    /// Tell the planet how hard it is being rained on, cell by cell, relative to the
    /// reference planet's mean.
    ///
    /// The one place anything outside this crate reaches in. Erosion without it is
    /// climate-blind and wears a desert down as fast as a rainforest — which is not a
    /// small error, because it is the land area that the carbon thermostat needs in order
    /// to work at all.
    pub fn set_runoff(&mut self, runoff: &[f32]) {
        debug_assert_eq!(runoff.len(), self.grid.len());
        match &mut self.runoff {
            Some(held) => held.copy_from_slice(runoff),
            none => *none = Some(runoff.to_vec()),
        }
    }

    /// What the last step's grid rounding had to shuffle. See [`Churn`].
    pub fn churn(&self) -> Churn {
        self.churn
    }

    pub fn plates(&self) -> &[Plate] {
        &self.plates
    }

    pub fn active_plates(&self) -> usize {
        self.plates.iter().filter(|p| p.active).count()
    }

    /// Share of the surface standing above the sea.
    pub fn land_fraction(&self) -> f32 {
        let land: f64 = self
            .grid
            .cells()
            .filter(|c| self.is_land(*c))
            .map(|c| self.area_km2[c as usize])
            .sum();
        let total: f64 = self.area_km2.iter().sum();
        (land / total) as f32
    }

    /// Share of the surface made of continental crust, drowned or not.
    pub fn continental_fraction(&self) -> f32 {
        let land: f64 = self
            .grid
            .cells()
            .filter(|c| self.crust[*c as usize] == CrustType::Continental)
            .map(|c| self.area_km2[c as usize])
            .sum();
        let total: f64 = self.area_km2.iter().sum();
        (land / total) as f32
    }

    /// The largest connected run of continental crust, as a share of all of it.
    ///
    /// The supercontinent index. One near unity means everything is in one mass; a
    /// scattered planet sits nearer a quarter.
    pub fn largest_landmass_share(&self) -> f32 {
        let mut seen = vec![false; self.grid.len()];
        let mut biggest = 0.0f64;
        let mut total = 0.0f64;
        let mut stack = Vec::new();
        for start in self.grid.cells() {
            if seen[start as usize] || self.crust[start as usize] != CrustType::Continental {
                continue;
            }
            let mut mass = 0.0;
            seen[start as usize] = true;
            stack.push(start);
            while let Some(cell) = stack.pop() {
                mass += self.area_km2[cell as usize];
                for &n in self.grid.neighbours(cell) {
                    if !seen[n as usize] && self.crust[n as usize] == CrustType::Continental {
                        seen[n as usize] = true;
                        stack.push(n);
                    }
                }
            }
            total += mass;
            biggest = biggest.max(mass);
        }
        if total == 0.0 {
            return 0.0;
        }
        (biggest / total) as f32
    }

    /// The cell containing a latitude and longitude, both in degrees.
    pub fn cell_at(&self, latitude: f64, longitude: f64) -> CellId {
        let (lat, lon) = (latitude.to_radians(), longitude.to_radians());
        let direction = Vec3::new(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin());
        self.grid.nearest_to(direction, 0)
    }

    // ---- running it ------------------------------------------------------------

    /// Advance the solid planet by a span of megayears.
    ///
    /// A megayear is the natural step: plates move less than a cell, crust ages by a
    /// readable amount, and erosion stays well inside its stability limit. Longer steps
    /// are accepted and will still conserve, but plates begin to jump cells and thin
    /// features are lost.
    pub fn step_myr(&mut self, dt: f32, rng: &mut Rng) {
        debug_assert!(dt > 0.0, "time only runs forwards");
        let mut left = dt;
        while left > 0.0 {
            let step = left.min(self.longest_safe_step());
            self.one_step(step, rng);
            left -= step;
        }
    }

    /// The longest span over which no plate moves far enough to jump a cell.
    ///
    /// Everything downstream assumes a parcel lands in its own cell or a neighbour: the
    /// rehousing search looks three rings out, and the divergent-gap rule assumes a gap
    /// is ringed by cells that are not gaps. Take a stride long enough to break that and
    /// the planet does not blow up, it degrades — crust is stranded, coastlines fray —
    /// which is worse, because it looks like it worked.
    fn longest_safe_step(&self) -> f32 {
        const SAFE_FRACTION: f64 = 0.25;
        let fastest = self
            .plates
            .iter()
            .filter(|p| p.active)
            .map(|p| p.rate.abs())
            .fold(0.0f64, f64::max);
        if fastest <= 0.0 {
            return f32::MAX;
        }
        // Rate is radians per megayear; a cell is `spacing / radius` radians across.
        (SAFE_FRACTION * self.cell_radians / fastest) as f32
    }

    fn one_step(&mut self, dt: f32, rng: &mut Rng) {
        for plate in self.plates.iter_mut().filter(|p| p.active) {
            plate.angle += plate.rate * dt as f64;
        }
        for age in self.crust_age_myr.iter_mut() {
            *age += dt;
        }

        self.drift(dt);
        self.spread(dt);
        self.classify_boundaries();
        self.magmatism(dt);
        self.settle();
        self.wear_down(dt);
        self.settle();
        self.reorganise(dt, rng);

        self.age_myr += dt as f64;
    }

    /// Carry every parcel to where its plate has taken it, and settle who ends up where.
    ///
    /// A **gather**, not a scatter: every cell asks which parcel has come nearest to it,
    /// rather than every parcel being thrown at a cell and the pile-ups sorted out
    /// afterwards. The difference matters more than it sounds. Scattering rounds a
    /// rotation into a permutation that is not quite a permutation, leaves holes and
    /// heaps that have to be reconciled by some rule, and every such rule is a small
    /// arbitrary displacement — repeated for a thousand steps, it is a random walk, and
    /// continents dissolve into an archipelago of single cells. Gathering asks a
    /// geometric question with a geometric answer, and the crust stays as coherent as it
    /// started.
    ///
    /// Two parcels can still want the same cell, and that is real: it means their plates
    /// are converging, and one of them is going down. A cell that no parcel reaches is
    /// equally real: the plates have parted and new ocean floor is born there.
    fn drift(&mut self, dt: f32) {
        let n = self.grid.len();

        for cell in 0..n {
            let plate = &self.plates[self.plate_of[cell] as usize];
            self.present[cell] = plate.present(self.frame[cell]);
        }

        // Match cells to parcels, nearest pairing first.
        //
        // Greedy, and the order is the whole of it: a cell with a parcel sitting almost
        // exactly on it has no alternative worth considering, so it should choose before
        // a cell whose best option was half a cell away and has several. Letting cells
        // choose in index order instead means an early cell takes a parcel that a later
        // one needed, and the later one — in the middle of a continent, with crust all
        // around it — declares itself an oceanic rift. That failure converted four
        // percent of the surface to sea floor every step.
        self.churn = Churn::default();
        self.colliding.clear();
        for target in 0..n {
            self.want_gap[target] = self.closest(target).0;
        }
        self.order.clear();
        self.order.extend(0..n as CellId);
        self.order.sort_unstable_by(|a, b| {
            self.want_gap[*a as usize]
                .total_cmp(&self.want_gap[*b as usize])
                .then(a.cmp(b))
        });

        self.claimed.fill(NOWHERE);
        self.wanted.fill(NOWHERE);
        for i in 0..n {
            let target = self.order[i];
            let here = self.grid.position(target);
            let (mut nearest, mut nearest_at) = (f64::MAX, NOWHERE);
            let (mut rival, mut rival_at) = (f64::MAX, NOWHERE);

            for slot in self.near_start[target as usize]..self.near_start[target as usize + 1] {
                let parcel = self.near[slot as usize];
                if self.claimed[parcel as usize] != NOWHERE {
                    continue;
                }
                let away = self.present[parcel as usize].angle_to(here);
                if away >= self.cell_radians * REACH {
                    continue;
                }
                if away < nearest {
                    if nearest_at != NOWHERE
                        && self.plate_of[nearest_at as usize] != self.plate_of[parcel as usize]
                        && nearest < rival
                    {
                        rival = nearest;
                        rival_at = nearest_at;
                    }
                    nearest = away;
                    nearest_at = parcel;
                } else if away < rival
                    && nearest_at != NOWHERE
                    && self.plate_of[parcel as usize] != self.plate_of[nearest_at as usize]
                {
                    rival = away;
                    rival_at = parcel;
                }
            }

            if nearest_at == NOWHERE {
                // Nothing close is still going spare. Before calling this a rift, look
                // further: greedy matching leaves a couple of percent of cells stranded
                // for no physical reason at all, purely because a neighbour chose first,
                // and every one of those manufactured a patch of ocean floor in the
                // middle of a continent. A cell only rifts when there is genuinely no
                // crust left nearby to cover it.
                let mut spare = (f64::MAX, NOWHERE);
                for slot in self.near_start[target as usize]..self.near_start[target as usize + 1] {
                    let parcel = self.near[slot as usize];
                    if self.claimed[parcel as usize] != NOWHERE {
                        continue;
                    }
                    let away = self.present[parcel as usize].angle_to(here);
                    if away < spare.0 && away < self.cell_radians * STRETCH {
                        spare = (away, parcel);
                    }
                }
                if spare.1 != NOWHERE {
                    self.wanted[target as usize] = spare.1;
                    self.claimed[spare.1 as usize] = target;
                }
                continue;
            }
            // Two plates both have crust here. They are converging, and one is going
            // down — which is to say the loser stays unclaimed, and unless some other
            // cell can use it, it is gone.
            let winner = if rival_at != NOWHERE && rival < nearest * CONTEST {
                self.resolve(nearest_at, rival_at)
            } else {
                nearest_at
            };
            self.wanted[target as usize] = winner;
            self.claimed[winner as usize] = target;
        }

        for target in 0..n {
            match self.wanted[target] {
                NOWHERE => self.next_thickness[target] = f32::NAN,
                parcel => self.settle_parcel(target, parcel),
            }
        }

        // Anywhere still empty is either a genuine rift or a rounding artefact, and the
        // two want opposite answers. New ocean floor may only be born between plates
        // that are parting; a hole that opens in the middle of a plate is arithmetic,
        // not geology, and filling it with sea floor converts continents to ocean at a
        // percent a step until there is nothing left. Which it is can be read straight
        // off the neighbours: crust of two different plates around it means a boundary.
        for target in 0..n {
            if !self.next_thickness[target].is_nan() {
                continue;
            }
            let mut donor = NOWHERE;
            for &n in self.grid.neighbours(target as CellId) {
                if !self.next_thickness[n as usize].is_nan() {
                    donor = n;
                    break;
                }
            }
            // Two rings, not one. A ridge gap opens *between* the plates that made it,
            // so the far side is often two cells away and a one-ring test calls the
            // whole ridge a rounding artefact — which quietly switches off seafloor
            // spreading, and with it the conveyor that keeps ocean floor young.
            let boundary = donor != NOWHERE && {
                let mine = self.next_plate[donor as usize];
                (self.near_start[target]..self.near_start[target + 1]).any(|slot| {
                    let n = self.near[slot as usize];
                    !self.next_thickness[n as usize].is_nan() && self.next_plate[n as usize] != mine
                })
            };
            let Some(donor) = (donor != NOWHERE).then_some(donor) else {
                // Ringed entirely by holes, which the step subdivision rules out.
                self.next_plate[target] = self.plate_of[target];
                self.next_frame[target] = self.frame[target];
                self.next_crust[target] = self.crust[target];
                self.next_thickness[target] = self.thickness_km[target];
                self.next_age[target] = self.crust_age_myr[target];
                self.next_sediment[target] = self.sediment_m[target];
                continue;
            };

            let plate = self.next_plate[donor as usize];
            self.next_plate[target] = plate;
            self.next_frame[target] =
                self.plates[plate as usize].frame_of(self.grid.position(target as CellId));
            if boundary {
                // Seafloor spreading: the only way crust is ever created.
                self.churn.opened += 1;
                self.next_crust[target] = CrustType::Oceanic;
                self.next_thickness[target] = OCEANIC_THICKNESS_KM;
                self.next_age[target] = 0.0;
                self.next_sediment[target] = 0.0;
            } else {
                // Inside a plate. Take after the neighbours; the hole was never real.
                self.churn.patched += 1;
                self.next_crust[target] = self.next_crust[donor as usize];
                self.next_thickness[target] = self.next_thickness[donor as usize];
                self.next_age[target] = self.next_age[donor as usize];
                self.next_sediment[target] = self.next_sediment[donor as usize];
            }
        }
        self.churn.stranded = (0..n).filter(|c| self.claimed[*c] == NOWHERE).count();

        for pair in std::mem::take(&mut self.colliding) {
            *self.pressure.entry(pair).or_insert(0.0) += dt;
        }
        self.colliding.clear();

        std::mem::swap(&mut self.plate_of, &mut self.next_plate);
        std::mem::swap(&mut self.frame, &mut self.next_frame);
        std::mem::swap(&mut self.crust, &mut self.next_crust);
        std::mem::swap(&mut self.thickness_km, &mut self.next_thickness);
        std::mem::swap(&mut self.crust_age_myr, &mut self.next_age);
        std::mem::swap(&mut self.sediment_m, &mut self.next_sediment);
    }

    /// How near the nearest parcel of any plate has come to this cell, and which it is.
    fn closest(&self, target: usize) -> (f64, CellId) {
        let here = self.grid.position(target as CellId);
        let mut best = (f64::MAX, NOWHERE);
        for slot in self.near_start[target]..self.near_start[target + 1] {
            let parcel = self.near[slot as usize];
            let away = self.present[parcel as usize].angle_to(here);
            if away < best.0 {
                best = (away, parcel);
            }
        }
        best
    }

    /// Two parcels want the same cell. Decide which survives, and what the meeting does
    /// to it. Returns the survivor.
    fn resolve(&mut self, a: CellId, b: CellId) -> CellId {
        let (pa, pb) = (self.plate_of[a as usize], self.plate_of[b as usize]);
        let (ca, cb) = (self.crust[a as usize], self.crust[b as usize]);

        if pa == pb {
            // Same plate: a rigid rotation cannot compress its own crust, so this is
            // the grid rounding two parcels into one cell, not a collision. Keep the
            // thicker and hand the other back — it is not merged, because inventing
            // mountains out of a rounding artefact is how a model starts lying, and it
            // is not discarded either, because that is how a plate eats itself. The
            // caller finds it a neighbouring cell.
            return if self.thickness_km[a as usize] >= self.thickness_km[b as usize] {
                a
            } else {
                b
            };
        }

        match (ca, cb) {
            // Ocean dives under continent, and is gone. What the melting slab gives
            // back is handled by `magmatism`, which works from the boundary itself
            // rather than from whether two parcels happened to round into one cell.
            (CrustType::Continental, CrustType::Oceanic) => a,
            (CrustType::Oceanic, CrustType::Continental) => b,
            // Neither will go down: continental crust is too buoyant to subduct. So it
            // piles up, and piled-up crust is a mountain range by isostasy alone.
            (CrustType::Continental, CrustType::Continental) => {
                let (keep, gone) = if self.plate_of[a as usize] <= self.plate_of[b as usize] {
                    (a, b)
                } else {
                    (b, a)
                };
                let added = self.thickness_km[gone as usize];
                self.thicken(keep, added);
                // Noted, not counted. Welding is a matter of how *long* two continents
                // have been colliding, not of how many cells collided this instant —
                // adding a step's worth per cell made a boundary a hundred cells long
                // weld in two steps, and the planet was down to three plates before the
                // first hundred megayears were out.
                self.colliding
                    .insert(if pa < pb { (pa, pb) } else { (pb, pa) });
                keep
            }
            // Older ocean floor is colder, denser, and goes down first.
            (CrustType::Oceanic, CrustType::Oceanic) => {
                if self.crust_age_myr[a as usize] <= self.crust_age_myr[b as usize] {
                    a
                } else {
                    b
                }
            }
        }
    }

    /// Put a parcel into a cell of the next state.
    fn settle_parcel(&mut self, target: usize, parcel: CellId) {
        self.next_plate[target] = self.plate_of[parcel as usize];
        self.next_frame[target] = self.frame[parcel as usize];
        self.next_crust[target] = self.crust[parcel as usize];
        self.next_thickness[target] = self.thickness_km[parcel as usize];
        self.next_age[target] = self.crust_age_myr[parcel as usize];
        self.next_sediment[target] = self.sediment_m[parcel as usize];
    }

    fn thicken(&mut self, cell: CellId, by: f32) {
        let t = &mut self.thickness_km[cell as usize];
        *t = (*t + by).min(MAX_CRUST_KM);
    }

    /// New crust rising off the subducting slab.
    ///
    /// Works from the boundary rather than from the collisions, because a trench is a
    /// trench whether or not two parcels happened to round into the same cell this step.
    /// Over an island arc it builds ocean floor towards the thickness at which it stops
    /// being ocean floor at all; over a continental margin it is the Andes.
    fn magmatism(&mut self, dt: f32) {
        for cell in 0..self.grid.len() {
            if self.boundary[cell] != Boundary::Convergent {
                continue;
            }
            self.thicken(cell as CellId, ARC_GROWTH_KM_PER_MYR * dt);
            if self.crust[cell].is_oceanic() && self.thickness_km[cell] >= ARC_MATURES_AT_KM {
                self.crust[cell] = CrustType::Continental;
            }
        }
    }

    /// Overthick crust flowing sideways under its own weight.
    ///
    /// The counterweight to collision. A cell that has been piled past what it can
    /// support pushes its excess into whichever neighbour is thinnest — widening the
    /// continent if that neighbour is ocean floor, which is the only way an orogen's
    /// bulk ever becomes area again.
    fn spread(&mut self, dt: f32) {
        for cell in 0..self.grid.len() {
            if self.crust[cell] != CrustType::Continental {
                continue;
            }
            let excess = self.thickness_km[cell] - COLLAPSE_AT_KM;
            if excess <= 0.0 {
                continue;
            }
            let Some(&downhill) = self.grid.neighbours(cell as CellId).iter().min_by(|a, b| {
                self.thickness_km[**a as usize].total_cmp(&self.thickness_km[**b as usize])
            }) else {
                continue;
            };
            // Only downhill, and never past the neighbour it is flowing into: crust
            // does not pump itself uphill.
            let gap = self.thickness_km[cell] - self.thickness_km[downhill as usize];
            if gap <= 0.0 {
                continue;
            }
            let flow = (excess * COLLAPSE_RATE * dt).min(gap * 0.5);
            self.thickness_km[cell] -= flow;
            self.thickness_km[downhill as usize] += flow;
            // Continental crust spread over ocean floor is continental crust — but a
            // continent widens along its own edge, it does not sprout islands in open
            // water. Requiring the receiving cell to already have a coast on two sides
            // is what keeps the margin a margin.
            let coastal = self
                .grid
                .neighbours(downhill)
                .iter()
                .filter(|n| self.crust[**n as usize] == CrustType::Continental)
                .count();
            if coastal >= 2 {
                self.crust[downhill as usize] = CrustType::Continental;
            }
        }
    }

    /// What kind of boundary each cell sits on, from the relative motion across it.
    ///
    /// Derived, not remembered. Whether two plates are converging is a fact about their
    /// velocities at that point, and reading it off directly means a plate
    /// reorganisation changes every boundary it should change, immediately, with
    /// nothing to keep in step.
    fn classify_boundaries(&mut self) {
        for cell in 0..self.grid.len() {
            let mine = self.plate_of[cell];
            let here = self.grid.position(cell as CellId);
            let mut worst = Boundary::Interior;
            let mut strongest = 0.0f64;

            for &n in self.grid.neighbours(cell as CellId) {
                let theirs = self.plate_of[n as usize];
                if theirs == mine {
                    continue;
                }
                let there = self.grid.position(n);
                let a = &self.plates[mine as usize];
                let b = &self.plates[theirs as usize];
                let va = a.pole.scaled(a.rate).cross(here);
                let vb = b.pole.scaled(b.rate).cross(there);
                let relative = va.minus(vb);

                // Split the relative motion into the part along the line between the
                // two cells and the part across it. Closing is convergence, opening is
                // divergence, and sliding past is a transform.
                let along = there.minus(here).normalised();
                let closing = -relative.dot(along);
                let total = relative.length();
                if total <= strongest {
                    continue;
                }
                strongest = total;
                worst = if closing.abs() < total * 0.5 {
                    Boundary::Transform
                } else if closing > 0.0 {
                    Boundary::Convergent
                } else {
                    Boundary::Divergent
                };
            }
            self.boundary[cell] = worst;
        }
    }

    /// Recompute elevation from crust, then find where the sea comes to rest.
    fn settle(&mut self) {
        for cell in 0..self.grid.len() {
            self.elevation_m[cell] = crust::elevation_m(
                self.crust[cell],
                self.thickness_km[cell],
                self.crust_age_myr[cell],
                self.sediment_m[cell],
            );
        }
        self.sea_level_m = crust::sea_level_m(&self.elevation_m, &self.area_km2, self.water_km3);
    }

    fn wear_down(&mut self, dt: f32) {
        self.erosion.wear_down(
            &self.grid,
            &self.area_km2,
            &self.elevation_m,
            self.sea_level_m,
            dt,
            self.radius_km,
            crust::BUOYANCY,
            self.runoff.as_deref(),
            &mut self.stripped_m,
            &mut self.deposited_m,
        );

        for cell in 0..self.grid.len() {
            // Loose sediment goes first; it is what is lying on top. Only what is left
            // over comes out of the bedrock beneath.
            let mut cut = self.stripped_m[cell];
            let from_cover = cut.min(self.sediment_m[cell]);
            self.sediment_m[cell] -= from_cover;
            cut -= from_cover;
            if cut > 0.0 {
                self.thickness_km[cell] = (self.thickness_km[cell] - cut / 1000.0).max(1.0);
            }
            self.sediment_m[cell] += self.deposited_m[cell];
        }
    }

    /// Plates changing their minds, welding together, and tearing apart.
    fn reorganise(&mut self, dt: f32, rng: &mut Rng) {
        // A slab tears, a continent jams a trench, and a plate's drive changes. Real
        // reorganisations are every fifty to a hundred megayears.
        for i in 0..self.plates.len() {
            if !self.plates[i].active {
                continue;
            }
            if rng.chance((dt as f64 / REORGANISATION_INTERVAL_MYR).min(1.0)) {
                // The crust must not move when the drive changes, so every parcel
                // re-addresses itself against the new pole first.
                let old = self.plates[i].clone();
                self.plates[i].redirect(rng);
                self.plates[i].angle = 0.0;
                let fresh = self.plates[i].clone();
                for cell in 0..self.grid.len() {
                    if self.plate_of[cell] == i as PlateId {
                        self.frame[cell] = fresh.frame_of(old.present(self.frame[cell]));
                    }
                }
            }
        }

        self.weld();
        self.rift(dt, rng);
    }

    /// Two continents that have been colliding long enough are one plate.
    ///
    /// This is where a supercontinent comes from. Nothing decides to build one; plates
    /// that happen to be converging on continental crust accumulate collision, and past
    /// a threshold the boundary between them stops being a boundary.
    fn weld(&mut self) {
        let ready: Vec<(PlateId, PlateId)> = self
            .pressure
            .iter()
            .filter(|(_, force)| **force >= WELD_AT)
            .map(|(pair, _)| *pair)
            .collect();

        for (keep, gone) in ready {
            self.pressure.remove(&(keep, gone));
            if !self.plates[keep as usize].active || !self.plates[gone as usize].active {
                continue;
            }
            let absorbed = self.plates[gone as usize].clone();
            let survivor = self.plates[keep as usize].clone();
            for cell in 0..self.grid.len() {
                if self.plate_of[cell] == gone {
                    // Re-addressed into the surviving plate's frame, so the crust does
                    // not so much as twitch at the moment of welding.
                    self.frame[cell] = survivor.frame_of(absorbed.present(self.frame[cell]));
                    self.plate_of[cell] = keep;
                }
            }
            self.plates[gone as usize].active = false;
            // Whatever the absorbed plate was pressing against, the survivor now is.
            let inherited: Vec<((PlateId, PlateId), f32)> = self
                .pressure
                .iter()
                .filter(|(pair, _)| pair.0 == gone || pair.1 == gone)
                .map(|(pair, force)| (*pair, *force))
                .collect();
            for (pair, force) in inherited {
                self.pressure.remove(&pair);
                let other = if pair.0 == gone { pair.1 } else { pair.0 };
                if other == keep {
                    continue;
                }
                let fresh = if keep < other {
                    (keep, other)
                } else {
                    (other, keep)
                };
                *self.pressure.entry(fresh).or_insert(0.0) += force;
            }
        }
    }

    /// A plate large enough to insulate the mantle beneath it splits.
    ///
    /// The other half of the cycle. A supercontinent traps heat, the trapped heat lifts
    /// and stretches the crust above it, and the crust fails along a line. The line here
    /// is a great circle through the plate's centre of mass, which is crude, but the
    /// timing and the consequence are not: the halves get their own poles and start
    /// moving apart, and the gap between them fills with new ocean floor by the ordinary
    /// spreading rule.
    fn rift(&mut self, dt: f32, rng: &mut Rng) {
        if self.active_plates() >= MAX_PLATES {
            return;
        }
        let continental: f64 = self
            .grid
            .cells()
            .filter(|c| self.crust[*c as usize] == CrustType::Continental)
            .map(|c| self.area_km2[c as usize])
            .sum();
        if continental <= 0.0 {
            return;
        }

        for candidate in 0..self.plates.len() {
            if !self.plates[candidate].active {
                continue;
            }
            let mine: f64 = self
                .grid
                .cells()
                .filter(|c| {
                    self.plate_of[*c as usize] == candidate as PlateId
                        && self.crust[*c as usize] == CrustType::Continental
                })
                .map(|c| self.area_km2[c as usize])
                .sum();
            if (mine / continental) as f32 <= RIFT_THRESHOLD_SHARE {
                continue;
            }
            if !rng.chance((dt as f64 / RIFT_INTERVAL_MYR).min(1.0)) {
                continue;
            }

            // Centre of mass of the plate's continental crust, and a great circle
            // through it in a random direction: the crack.
            let mut centre = Vec3::new(0.0, 0.0, 0.0);
            for cell in self.grid.cells() {
                if self.plate_of[cell as usize] == candidate as PlateId {
                    centre = centre.plus(
                        self.grid
                            .position(cell)
                            .scaled(self.area_km2[cell as usize]),
                    );
                }
            }
            let centre = centre.normalised();
            let cut = plates::random_direction(rng).cross(centre).normalised();

            let Some(fresh) = self.plates.iter().position(|p| !p.active) else {
                if self.plates.len() >= MAX_PLATES {
                    return;
                }
                self.plates.push(Plate::random(rng));
                let fresh = self.plates.len() - 1;
                self.split(candidate, fresh, cut);
                return;
            };
            self.plates[fresh] = Plate::random(rng);
            self.split(candidate, fresh, cut);
            return;
        }
    }

    /// Move every parcel of `from` on the far side of `cut` onto plate `into`.
    fn split(&mut self, from: usize, into: usize, cut: Vec3) {
        let old = self.plates[from].clone();
        let new = self.plates[into].clone();
        let mut moved = 0usize;
        for cell in 0..self.grid.len() {
            if self.plate_of[cell] != from as PlateId {
                continue;
            }
            if old.present(self.frame[cell]).dot(cut) <= 0.0 {
                continue;
            }
            self.frame[cell] = new.frame_of(old.present(self.frame[cell]));
            self.plate_of[cell] = into as PlateId;
            moved += 1;
        }
        // A cut that caught nothing leaves a plate with no crust, which would then
        // never be reused because it looks active.
        self.plates[into].active = moved > 0;
    }
}

/// Each cell, its neighbours, and their neighbours, flattened with start offsets.
///
/// The search radius for the gather. A parcel moves at most a quarter of a cell in a
/// step, so one ring would very nearly do; two costs a few megabytes and removes the
/// question.
fn two_rings(grid: &Grid) -> (Vec<CellId>, Vec<u32>) {
    let mut near = Vec::with_capacity(grid.len() * 19);
    let mut start = Vec::with_capacity(grid.len() + 1);
    let mut group: Vec<CellId> = Vec::with_capacity(19);
    for cell in grid.cells() {
        start.push(near.len() as u32);
        group.clear();
        group.push(cell);
        for &n in grid.neighbours(cell) {
            if !group.contains(&n) {
                group.push(n);
            }
            for &far in grid.neighbours(n) {
                if !group.contains(&far) {
                    group.push(far);
                }
            }
        }
        near.extend_from_slice(&group);
    }
    start.push(near.len() as u32);
    (near, start)
}

/// How many hops each cell is from the nearest cell satisfying `is_source`.
///
/// A plain multi-source breadth-first sweep. Used at genesis to build the two fields
/// that have to vary smoothly across the surface rather than cell by cell.
fn hops_from(grid: &Grid, is_source: impl Fn(CellId) -> bool) -> Vec<u32> {
    let mut distance = vec![u32::MAX; grid.len()];
    let mut queue: Vec<CellId> = grid.cells().filter(|c| is_source(*c)).collect();
    for cell in &queue {
        distance[*cell as usize] = 0;
    }
    let mut at = 0;
    while at < queue.len() {
        let cell = queue[at];
        at += 1;
        let next = distance[cell as usize] + 1;
        for &n in grid.neighbours(cell) {
            if distance[n as usize] == u32::MAX {
                distance[n as usize] = next;
                queue.push(n);
            }
        }
    }
    // A planet with no continent at all, or no ocean: everything is equally far from
    // something that is not there.
    for d in distance.iter_mut() {
        if *d == u32::MAX {
            *d = 0;
        }
    }
    distance
}

/// Assign every cell to the nearest of `count` seeds, by breadth-first growth.
///
/// Grown rather than measured: a nearest-seed partition by distance gives smooth
/// circular plates, and real plate outlines are ragged. Multi-source growth over the
/// cell graph gives boundaries that wander.
fn flood_fill_plates(grid: &Grid, count: usize, rng: &mut Rng) -> Vec<PlateId> {
    let n = grid.len();
    let mut owner = vec![PlateId::MAX; n];
    let mut frontier: Vec<(PlateId, CellId)> = Vec::new();

    for plate in 0..count {
        let seed = rng.range_u64(0, n as u64 - 1) as CellId;
        if owner[seed as usize] != PlateId::MAX {
            continue;
        }
        owner[seed as usize] = plate as PlateId;
        frontier.push((plate as PlateId, seed));
    }

    while !frontier.is_empty() {
        // Drawing from the frontier at random rather than in order is what keeps the
        // boundaries irregular; taking it as a queue would give tidy circles again.
        let pick = rng.range_u64(0, frontier.len() as u64 - 1) as usize;
        let (plate, cell) = frontier.swap_remove(pick);
        for &next in grid.neighbours(cell) {
            if owner[next as usize] == PlateId::MAX {
                owner[next as usize] = plate;
                frontier.push((plate, next));
            }
        }
    }
    owner
}

/// Grow continents from random cratons until they cover the requested share.
fn grow_continents(grid: &Grid, fraction: f32, rng: &mut Rng) -> Vec<bool> {
    let n = grid.len();
    let mut land = vec![false; n];
    let target = ((n as f32) * fraction.clamp(0.0, 1.0)) as usize;
    if target == 0 {
        return land;
    }

    // Few big cratons and a scatter of small ones, which is the size distribution real
    // continents have; equal-sized blobs look manufactured.
    let cratons = (target / 120).clamp(2, 9);
    let mut frontier: Vec<CellId> = Vec::new();
    let mut count = 0usize;
    for _ in 0..cratons {
        let seed = rng.range_u64(0, n as u64 - 1) as CellId;
        if !land[seed as usize] {
            land[seed as usize] = true;
            count += 1;
            frontier.push(seed);
        }
    }

    while count < target && !frontier.is_empty() {
        let pick = rng.range_u64(0, frontier.len() as u64 - 1) as usize;
        let cell = frontier[pick];
        let free: Vec<CellId> = grid
            .neighbours(cell)
            .iter()
            .copied()
            .filter(|c| !land[*c as usize])
            .collect();
        match free.first() {
            None => {
                frontier.swap_remove(pick);
            }
            Some(&next) => {
                land[next as usize] = true;
                count += 1;
                frontier.push(next);
            }
        }
    }
    land
}

#[cfg(test)]
mod tests;
