//! Rivers taking mountains apart and putting them in basins.
//!
//! One process, not two. Water runs downhill, gathers as it goes, and cuts in
//! proportion to how much of it there is and how steeply it falls — the stream-power
//! law, `E = K·√A·S`. Everything a landscape does at this scale follows from that plus
//! the bookkeeping: what is cut out of a mountain has to end up somewhere, so the same
//! pass that erodes carries the debris down the drainage and drops it in the first
//! basin or sea it reaches.
//!
//! **Hillslope diffusion is deliberately absent**, though the design lists it. Real
//! hillslope creep has a diffusivity around 10⁻² m²/yr; at a cell spacing of 112 km the
//! term it contributes is smaller than a millimetre per megayear, which is to say it is
//! sub-grid by four orders of magnitude. Including it would mean inventing a
//! coefficient thousands of times the measured one and calling the result physics.
//! Stream power is the term that matters at continental scale, and it is the one that
//! is here.

use crate::grid::{CellId, Grid};

/// Erodibility, in metres of rock per megayear per (√km² of drainage × unit slope).
///
/// This is the literature coefficient in the units this module works in. Bedrock
/// incision studies put `K` around 10⁻⁸ per year with area in square metres; converting
/// to square kilometres of drainage and megayears of time multiplies it by 10⁹, which is
/// where the ten comes from. The check on it is denudation rate: a million square
/// kilometres draining across a two-percent gradient gives about two hundred metres of
/// rock per megayear, or 0.2 mm a year, which is what measured active orogens do.
const STREAM_POWER: f64 = 10.0;

/// The stream power a channel needs before it cuts rock at all, in the same units as
/// `√A·S` above.
///
/// Threshold stream power, the standard form: `E = K(√A·S − ω)`. Without the threshold a
/// continental interior at a gradient of one part in a thousand still erodes at tens of
/// metres per megayear, which planes every continent down to the waterline inside a few
/// hundred megayears and leaves a planet with almost no land — which is exactly what
/// this model did before the term was here. With it, a craton is below threshold and
/// essentially permanent, while an active mountain front is far above it and comes down
/// on the timescale orogens actually come down on. The value is set by that contrast,
/// which is the observable the threshold exists to reproduce.
const INCISION_THRESHOLD: f64 = 1.0;

/// No cell may cut more than this fraction of the way down to the cell it drains into.
///
/// Without it a single long step can erode a cell below its own outlet, which reverses
/// the flow direction and makes the drainage network oscillate.
const MAX_INCISION_FRACTION: f64 = 0.5;

/// Reusable scratch for the erosion pass. Held across steps so a megayear of landscape
/// evolution allocates nothing.
pub struct Erosion {
    order: Vec<CellId>,
    receiver: Vec<CellId>,
    /// Drainage area, km².
    discharge: Vec<f64>,
    /// Sediment in transit, m³.
    load: Vec<f64>,
}

/// Where a cell's water goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Flow {
    /// Downhill, to this neighbour.
    To(CellId),
    /// Nowhere: a closed basin, or already under water.
    Rests,
}

impl Erosion {
    pub fn new(cells: usize) -> Erosion {
        Erosion {
            order: Vec::with_capacity(cells),
            receiver: vec![0; cells],
            discharge: vec![0.0; cells],
            load: vec![0.0; cells],
        }
    }

    /// Run one span of landscape evolution.
    ///
    /// Writes how much *rock* each cell lost into `stripped_m`, and how much settled on
    /// it into `deposited_m`. Rock, not height: strip a metre off a mountain and the
    /// mountain does not get a metre shorter, because the root under it rises. How much
    /// shorter it does get is `surface_per_rock` — isostasy's answer, which the caller
    /// supplies because this module deliberately knows nothing about crust. It is needed
    /// here only to keep a cell from cutting itself below its own outlet.
    #[allow(clippy::too_many_arguments)]
    pub fn wear_down(
        &mut self,
        grid: &Grid,
        areas_km2: &[f64],
        elevation: &[f32],
        sea_level: f32,
        dt_myr: f32,
        radius_km: f64,
        surface_per_rock: f32,
        // `runoff`: rainfall at each cell relative to the reference planet's mean, or
        // `None` for a world that has no climate yet and is rained on evenly.
        runoff: Option<&[f32]>,
        stripped_m: &mut [f32],
        deposited_m: &mut [f32],
    ) {
        let n = grid.len();
        stripped_m.fill(0.0);
        deposited_m.fill(0.0);
        self.load.fill(0.0);

        // Highest first. Every cell's uphill contributions are therefore complete
        // before it is reached, which is what lets one pass do accumulation, incision,
        // and deposition together.
        self.order.clear();
        self.order.extend(grid.cells());
        self.order.sort_unstable_by(|a, b| {
            elevation[*b as usize]
                .total_cmp(&elevation[*a as usize])
                // Ties settled by index. Two cells at exactly the same height is common
                // on a fresh ocean floor, and leaving the order to the sort would let
                // it vary between runs of the same seed.
                .then(a.cmp(b))
        });

        for (cell, area) in areas_km2.iter().enumerate().take(n) {
            // Discharge, not drainage area. A square kilometre of rainforest delivers a
            // river and a square kilometre of Sahara delivers nothing, and weighting the
            // accumulation by how much actually falls is what makes a desert erode
            // slowly. Without it the model wears its continents down at the same rate
            // everywhere, which drowns them: land falls away, weathering loses the rock
            // it works on, and the carbon cycle has nothing left to regulate with.
            self.discharge[cell] = *area * runoff.map_or(1.0, |r| r[cell].max(0.02) as f64);
            self.receiver[cell] = cell as CellId;
        }

        for i in 0..self.order.len() {
            let cell = self.order[i];
            let here = elevation[cell as usize];
            let flow = self.steepest_descent(grid, elevation, cell);

            let target = match flow {
                Flow::To(next) if here > sea_level => next,
                // Under water, or in a hole with no way out. Whatever arrived, stays.
                _ => {
                    let settled = self.load[cell as usize];
                    deposited_m[cell as usize] +=
                        (settled / (areas_km2[cell as usize] * 1.0e6)) as f32;
                    self.load[cell as usize] = 0.0;
                    continue;
                }
            };
            self.receiver[cell as usize] = target;

            // A river's gradient runs to the waterline, not to the sea floor. Measuring
            // it to the bottom of the abyss instead makes every coastal cell look like
            // it sits on a seven-kilometre cliff, and erodes continents away in tens of
            // megayears rather than billions.
            let base = elevation[target as usize].max(sea_level);
            let drop = (here - base) as f64;
            let span = grid.distance_km(cell, target, radius_km).max(1.0);
            let slope = (drop / (span * 1000.0)).max(0.0);

            // The cap is on the *height* this removes, so it converts through isostasy
            // before being compared with the drop.
            let headroom = (drop * MAX_INCISION_FRACTION / surface_per_rock as f64).max(0.0);
            let power = self.discharge[cell as usize].sqrt() * slope;
            let cut = (STREAM_POWER * (power - INCISION_THRESHOLD).max(0.0) * dt_myr as f64)
                .min(headroom)
                .max(0.0);

            stripped_m[cell as usize] = cut as f32;
            self.load[cell as usize] += cut * areas_km2[cell as usize] * 1.0e6;

            // Hand water and debris downstream together.
            self.discharge[target as usize] += self.discharge[cell as usize];
            self.load[target as usize] += self.load[cell as usize];
            self.load[cell as usize] = 0.0;
        }
    }

    /// Drainage area at a cell, km². Valid after [`Erosion::wear_down`].
    pub fn discharge_km2(&self, cell: CellId) -> f64 {
        self.discharge[cell as usize]
    }

    /// The cell this one drains into, or itself if it drains nowhere.
    pub fn receiver(&self, cell: CellId) -> CellId {
        self.receiver[cell as usize]
    }

    fn steepest_descent(&self, grid: &Grid, elevation: &[f32], cell: CellId) -> Flow {
        let here = elevation[cell as usize];
        let mut best = Flow::Rests;
        let mut lowest = here;
        for &n in grid.neighbours(cell) {
            let there = elevation[n as usize];
            if there < lowest {
                lowest = there;
                best = Flow::To(n);
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grid with a single mountain: elevation falls away from cell zero.
    ///
    /// The range spans a few percent of the sphere rather than the whole of it, and the
    /// tests run a level finer than the planet ones. A cone stretched pole to pole has a
    /// gradient of a few parts in ten thousand — flatter than any real landscape, below
    /// the incision threshold everywhere, and so it simply does not erode. A test built
    /// on one is measuring the shape of the test.
    fn cone(grid: &Grid, height: f32) -> Vec<f32> {
        const REACH: f64 = 0.06;
        let peak = grid.position(0);
        grid.cells()
            .map(|c| {
                let away = grid.position(c).angle_to(peak) / std::f64::consts::PI;
                (height as f64 * (1.0 - away / REACH)).max(-4000.0) as f32
            })
            .collect()
    }

    fn areas(grid: &Grid) -> Vec<f64> {
        grid.cells().map(|c| grid.area_km2(c, 6371.0)).collect()
    }

    #[test]
    fn water_gathers_downhill() {
        // Total drainage has to add up to the whole surface, or the accumulation is
        // dropping water somewhere.
        let grid = Grid::new(4);
        let areas = areas(&grid);
        let elevation = cone(&grid, 4000.0);
        let mut erosion = Erosion::new(grid.len());
        let mut stripped = vec![0.0; grid.len()];
        let mut deposited = vec![0.0; grid.len()];
        erosion.wear_down(
            &grid,
            &areas,
            &elevation,
            0.0,
            1.0,
            6371.0,
            1.0,
            None,
            &mut stripped,
            &mut deposited,
        );

        // Every drop that falls on land has to reach the sea exactly once. A radial
        // mountain has many outlets rather than one, so the claim is not about any
        // single cell — it is that the water adds up: what the coastal cells deliver
        // to the sea is the whole of the land, no more and no less.
        let dry: f64 = grid
            .cells()
            .filter(|c| elevation[*c as usize] > 0.0)
            .map(|c| areas[c as usize])
            .sum();
        let delivered: f64 = grid
            .cells()
            .filter(|c| elevation[*c as usize] > 0.0)
            .filter(|c| elevation[erosion.receiver(*c) as usize] <= 0.0)
            .map(|c| erosion.discharge_km2(c))
            .sum();
        assert!(
            (delivered - dry).abs() / dry < 1e-9,
            "the coast delivered {delivered:.0} km² of {dry:.0} km² of land"
        );

        // And accumulation is monotonic: nowhere does a river get smaller going down.
        for cell in grid.cells().filter(|c| elevation[*c as usize] > 0.0) {
            let next = erosion.receiver(cell);
            if next != cell {
                assert!(
                    erosion.discharge_km2(next) > erosion.discharge_km2(cell),
                    "flow shrank between {cell} and {next}"
                );
            }
        }
    }

    #[test]
    fn mountains_come_down_and_the_sea_floor_does_not() {
        let grid = Grid::new(4);
        let areas = areas(&grid);
        let elevation = cone(&grid, 4000.0);
        let mut erosion = Erosion::new(grid.len());
        let mut stripped = vec![0.0; grid.len()];
        let mut deposited = vec![0.0; grid.len()];
        erosion.wear_down(
            &grid,
            &areas,
            &elevation,
            0.0,
            1.0,
            6371.0,
            1.0,
            None,
            &mut stripped,
            &mut deposited,
        );

        for cell in grid.cells() {
            if elevation[cell as usize] <= 0.0 {
                assert_eq!(
                    stripped[cell as usize], 0.0,
                    "cell {cell} eroded below the waves"
                );
            }
        }
        let high: f32 = grid
            .cells()
            .filter(|c| elevation[*c as usize] > 2000.0)
            .map(|c| stripped[c as usize])
            .sum();
        assert!(high > 0.0, "nothing on the mountain eroded at all");
    }

    #[test]
    fn what_is_cut_out_is_put_back_somewhere() {
        // Conservation. A landscape model that quietly loses rock will hollow out a
        // planet over five hundred megayears and nobody will see it happen.
        let grid = Grid::new(4);
        let areas = areas(&grid);
        let elevation = cone(&grid, 4000.0);
        let mut erosion = Erosion::new(grid.len());
        let mut stripped = vec![0.0; grid.len()];
        let mut deposited = vec![0.0; grid.len()];
        erosion.wear_down(
            &grid,
            &areas,
            &elevation,
            0.0,
            1.0,
            6371.0,
            1.0,
            None,
            &mut stripped,
            &mut deposited,
        );

        let cut: f64 = grid
            .cells()
            .map(|c| stripped[c as usize] as f64 * areas[c as usize])
            .sum();
        let laid: f64 = grid
            .cells()
            .map(|c| deposited[c as usize] as f64 * areas[c as usize])
            .sum();
        assert!(cut > 0.0, "nothing eroded, so there is nothing to conserve");
        assert!(
            (cut - laid).abs() / cut < 1e-6,
            "cut {cut:.0} but laid down {laid:.0}"
        );
    }

    #[test]
    fn erosion_never_reverses_a_slope() {
        // The step-size failure. If a cell can cut below its own outlet, the drainage
        // flips direction next step and the landscape oscillates forever.
        let grid = Grid::new(4);
        let areas = areas(&grid);
        let mut elevation = cone(&grid, 9000.0);
        let mut erosion = Erosion::new(grid.len());
        let mut stripped = vec![0.0; grid.len()];
        let mut deposited = vec![0.0; grid.len()];

        for _ in 0..40 {
            erosion.wear_down(
                &grid,
                &areas,
                &elevation,
                0.0,
                10.0,
                6371.0,
                1.0,
                None,
                &mut stripped,
                &mut deposited,
            );
            for cell in grid.cells() {
                let receiver = erosion.receiver(cell);
                if receiver != cell {
                    let after = elevation[cell as usize] - stripped[cell as usize];
                    assert!(
                        after >= elevation[receiver as usize],
                        "cell {cell} cut below its own outlet"
                    );
                }
            }
            for cell in grid.cells() {
                elevation[cell as usize] -= stripped[cell as usize];
            }
        }
    }

    #[test]
    fn a_range_wears_down_over_hundreds_of_megayears() {
        // The calibration claim, run as a claim: a five-kilometre range with nothing
        // renewing it loses most of its height over a few hundred megayears, which is
        // what the Appalachians did.
        //
        // Run a level finer than everything else here, and deliberately. The gradient a
        // grid can represent is bounded by its spacing, and orogenic gradients — a few
        // percent — need cells of a couple of hundred kilometres or less. On a coarser
        // grid the same range is a gentle ramp and erodes at cratonic rates, which is
        // not the model being wrong so much as the grid being unable to hold the
        // question.
        //
        // Measured across the range rather than at the summit: a drainage divide has
        // almost no water crossing it and is the last thing to go, so watching the peak
        // measures the one point that erodes least.
        let grid = Grid::new(5);
        let areas = areas(&grid);
        let mut elevation = ridge(&grid, 5000.0);
        let range: Vec<CellId> = grid
            .cells()
            .filter(|c| elevation[*c as usize] > 500.0)
            .collect();
        let mean = |e: &Vec<f32>| {
            range.iter().map(|c| e[*c as usize] as f64).sum::<f64>() / range.len() as f64
        };

        let mut erosion = Erosion::new(grid.len());
        let mut stripped = vec![0.0; grid.len()];
        let mut deposited = vec![0.0; grid.len()];
        let started = mean(&elevation);

        // The claim that is actually about stream power rather than about the constant
        // in front of it: cutting goes where the water is. Read off the first step,
        // while the cone still has a uniform gradient, so the only thing varying between
        // cells is how much drainage passes through them. Reading it at the *end*
        // measures the opposite, and correctly: by then the trunk valleys have graded
        // themselves flat and it is the dry divides that still have a slope left to lose.
        erosion.wear_down(
            &grid,
            &areas,
            &elevation,
            0.0,
            3.0,
            6371.0,
            crate::crust::BUOYANCY,
            None,
            &mut stripped,
            &mut deposited,
        );
        // Stream power says the cut is proportional to √A once the threshold is met, so
        // across a cone of uniform gradient the two should be very nearly a straight
        // line. Correlation rather than a ratio of buckets: a range only a few cells
        // across has too few cells for buckets to say anything.
        let flow: Vec<f64> = range
            .iter()
            .map(|c| erosion.discharge_km2(*c).sqrt())
            .collect();
        let cut: Vec<f64> = range.iter().map(|c| stripped[*c as usize] as f64).collect();
        let correlation = pearson(&flow, &cut);
        assert!(
            correlation > 0.8,
            "cutting tracked drainage at only r={correlation:.2} — erosion is not \
             following the water"
        );

        for _ in 0..100 {
            erosion.wear_down(
                &grid,
                &areas,
                &elevation,
                0.0,
                3.0,
                6371.0,
                // Rock removed becomes height lost through isostasy, at about a sixth.
                crate::crust::BUOYANCY,
                None,
                &mut stripped,
                &mut deposited,
            );
            for cell in grid.cells() {
                elevation[cell as usize] -= stripped[cell as usize] * crate::crust::BUOYANCY;
            }
        }
        let left = mean(&elevation);
        assert!(
            left < started * 0.95 && left > started * 0.4,
            "after 300 Myr the range averaged {left:.0} m of {started:.0} m"
        );
    }

    /// How closely two series move together, in the range −1 to 1.
    fn pearson(a: &[f64], b: &[f64]) -> f64 {
        let n = a.len() as f64;
        let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
        let mut top = 0.0;
        let (mut va, mut vb) = (0.0, 0.0);
        for (x, y) in a.iter().zip(b) {
            top += (x - ma) * (y - mb);
            va += (x - ma) * (x - ma);
            vb += (y - mb) * (y - mb);
        }
        if va == 0.0 || vb == 0.0 {
            return 0.0;
        }
        top / (va * vb).sqrt()
    }

    /// A compact range: steep enough that a grid at this spacing can see the gradient.
    fn ridge(grid: &Grid, height: f32) -> Vec<f32> {
        const REACH: f64 = 0.06;
        let crest = grid.position(0);
        grid.cells()
            .map(|c| {
                let away = grid.position(c).angle_to(crest) / std::f64::consts::PI;
                (height as f64 * (1.0 - away / REACH)).max(-4000.0) as f32
            })
            .collect()
    }

    #[test]
    fn a_flat_world_erodes_nothing() {
        let grid = Grid::new(4);
        let areas = areas(&grid);
        let elevation = vec![500.0; grid.len()];
        let mut erosion = Erosion::new(grid.len());
        let mut stripped = vec![0.0; grid.len()];
        let mut deposited = vec![0.0; grid.len()];
        erosion.wear_down(
            &grid,
            &areas,
            &elevation,
            0.0,
            1.0,
            6371.0,
            1.0,
            None,
            &mut stripped,
            &mut deposited,
        );
        assert!(stripped.iter().all(|l| *l == 0.0));
    }

    #[test]
    fn a_closed_basin_keeps_what_reaches_it() {
        // An interior sink above sea level fills rather than leaking. This is how the
        // Tarim and the Caspian come by their kilometres of fill, and it matters because
        // a model that quietly drains every basin to the sea cannot make one.
        let grid = Grid::new(4);
        let areas = areas(&grid);
        // A bowl: lowest at cell zero, rising in every direction, and the whole of it
        // well above the waterline so there is nowhere for the debris to escape to.
        let floor = grid.position(0);
        let elevation: Vec<f32> = grid
            .cells()
            .map(|c| {
                let away = grid.position(c).angle_to(floor) / std::f64::consts::PI;
                (500.0 + 4000.0 * away) as f32
            })
            .collect();

        let mut erosion = Erosion::new(grid.len());
        let mut stripped = vec![0.0; grid.len()];
        let mut deposited = vec![0.0; grid.len()];
        erosion.wear_down(
            &grid,
            &areas,
            &elevation,
            0.0,
            1.0,
            6371.0,
            1.0,
            None,
            &mut stripped,
            &mut deposited,
        );
        assert!(
            deposited[0] > 0.0,
            "the bottom of a closed bowl collected nothing"
        );
        let escaped: f32 = grid
            .cells()
            .filter(|c| *c != 0)
            .map(|c| deposited[c as usize])
            .sum();
        assert_eq!(
            escaped, 0.0,
            "sediment settled somewhere that is not the sink"
        );
    }

    #[test]
    fn the_same_landscape_erodes_the_same_way_twice() {
        let grid = Grid::new(4);
        let areas = areas(&grid);
        let elevation = cone(&grid, 4000.0);
        let run = || {
            let mut erosion = Erosion::new(grid.len());
            let mut stripped = vec![0.0; grid.len()];
            let mut deposited = vec![0.0; grid.len()];
            erosion.wear_down(
                &grid,
                &areas,
                &elevation,
                0.0,
                1.0,
                6371.0,
                1.0,
                None,
                &mut stripped,
                &mut deposited,
            );
            (stripped, deposited)
        };
        assert_eq!(run(), run());
    }
}
