//! An icosahedral geodesic sphere: the planet's addressable surface.
//!
//! Cells are the *vertices* of a subdivided icosahedron, which makes each cell's
//! territory the dual polygon around it — a hexagon almost everywhere, a pentagon at the
//! twelve original corners. Why not latitude and longitude: a lat/lon grid has cells
//! that shrink to nothing at the poles, a singularity where every meridian meets, and
//! twenty-to-one variation in area between the equator and 85°. Every one of those is a
//! bug generator for a diffusion equation. The geodesic has none of them; its cells vary
//! in area by about a fifth, and its worst defect — that twelve cells have five
//! neighbours instead of six — is a fact you can write down and test.
//!
//! | Level | Cells | Spacing on an Earth-sized planet |
//! | --- | --- | --- |
//! | 4 | 2,562 | ~450 km |
//! | 5 | 10,242 | ~220 km |
//! | 6 | 40,962 | ~112 km |
//! | 7 | 163,842 | ~56 km |
//!
//! Fields over the grid live in parallel arrays elsewhere, not in a per-cell struct: an
//! erosion sweep touches elevation and nothing else, and wants those forty thousand
//! floats contiguous.

use std::collections::HashMap;

use crate::sphere::{Vec3, triangle_area};

/// A cell index. Plain `u32` rather than a generational handle: unlike a person, a cell
/// is never created or destroyed after the grid is built, so there is no stale-handle
/// problem for a generation counter to catch.
pub type CellId = u32;

/// The most neighbours any cell has. Twelve cells have five.
pub const MAX_NEIGHBOURS: usize = 6;

const NONE: CellId = CellId::MAX;

/// A geodesic sphere of cells.
pub struct Grid {
    level: u8,
    positions: Vec<Vec3>,
    /// Flat, stride [`MAX_NEIGHBOURS`], padded with [`NONE`] at the pentagons.
    neighbours: Vec<CellId>,
    degrees: Vec<u8>,
    /// Each cell's share of the sphere, in steradians. Sums to 4π.
    solid_angles: Vec<f64>,
}

impl Grid {
    /// Build a grid by subdividing an icosahedron `level` times.
    ///
    /// Cost is linear in the cell count and the whole thing is immutable afterwards, so
    /// this is paid once per world.
    pub fn new(level: u8) -> Grid {
        let (positions, faces) = subdivided_icosahedron(level);
        let (neighbours, degrees) = adjacency(positions.len(), &faces);
        let solid_angles = dual_areas(&positions, &faces);
        Grid {
            level,
            positions,
            neighbours,
            degrees,
            solid_angles,
        }
    }

    pub fn level(&self) -> u8 {
        self.level
    }

    pub fn len(&self) -> usize {
        self.positions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    pub fn cells(&self) -> impl Iterator<Item = CellId> {
        0..self.len() as CellId
    }

    pub fn position(&self, cell: CellId) -> Vec3 {
        self.positions[cell as usize]
    }

    pub fn neighbours(&self, cell: CellId) -> &[CellId] {
        let start = cell as usize * MAX_NEIGHBOURS;
        &self.neighbours[start..start + self.degrees[cell as usize] as usize]
    }

    pub fn degree(&self, cell: CellId) -> usize {
        self.degrees[cell as usize] as usize
    }

    /// The cell's share of the sphere, in steradians.
    pub fn solid_angle(&self, cell: CellId) -> f64 {
        self.solid_angles[cell as usize]
    }

    /// The cell's area on a planet of the given radius, in square kilometres.
    pub fn area_km2(&self, cell: CellId, radius_km: f64) -> f64 {
        self.solid_angles[cell as usize] * radius_km * radius_km
    }

    /// Great-circle distance between two cells, in kilometres.
    pub fn distance_km(&self, a: CellId, b: CellId, radius_km: f64) -> f64 {
        self.position(a).angle_to(self.position(b)) * radius_km
    }

    /// The cell nearest a direction, starting the search from `hint`.
    ///
    /// Hill-climbing on the dot product, with the hint as the starting point. Plate
    /// motion moves material less than one cell per megayear, so passing last step's
    /// answer turns this into a couple of comparisons instead of a search.
    ///
    /// The walk widens to two rings before it gives up. A greedy walk over a true
    /// Delaunay triangulation cannot get stuck, but a subdivided icosahedron is not
    /// quite Delaunay near its twelve pentagons, and there the immediate neighbours can
    /// all be worse while a cell two hops away is better — which is a wrong answer, not
    /// a slow one, and showed up as exactly that.
    pub fn nearest_to(&self, direction: Vec3, hint: CellId) -> CellId {
        let mut best = hint.min(self.len() as CellId - 1);
        let mut best_dot = self.position(best).dot(direction);
        loop {
            let mut improved = None;
            for &n in self.neighbours(best) {
                let dot = self.position(n).dot(direction);
                if dot > best_dot {
                    best_dot = dot;
                    improved = Some(n);
                }
            }
            if improved.is_none() {
                'wider: for &n in self.neighbours(best) {
                    for &far in self.neighbours(n) {
                        let dot = self.position(far).dot(direction);
                        if dot > best_dot {
                            best_dot = dot;
                            improved = Some(far);
                            break 'wider;
                        }
                    }
                }
            }
            match improved {
                Some(n) => best = n,
                None => return best,
            }
        }
    }

    /// Mean spacing between neighbouring cells, in kilometres — the grid's resolution.
    pub fn spacing_km(&self, radius_km: f64) -> f64 {
        let mut total = 0.0;
        let mut count = 0usize;
        for cell in self.cells() {
            for &n in self.neighbours(cell) {
                total += self.distance_km(cell, n, radius_km);
                count += 1;
            }
        }
        total / count as f64
    }
}

/// The twelve vertices of a regular icosahedron, as three golden rectangles.
fn icosahedron() -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let phi = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let mut vertices = Vec::with_capacity(12);
    for &(a, b) in &[(1.0, phi), (-1.0, phi), (1.0, -phi), (-1.0, -phi)] {
        vertices.push(Vec3::new(0.0, a, b).normalised());
    }
    for &(a, b) in &[(1.0, phi), (-1.0, phi), (1.0, -phi), (-1.0, -phi)] {
        vertices.push(Vec3::new(a, b, 0.0).normalised());
    }
    for &(a, b) in &[(1.0, phi), (-1.0, phi), (1.0, -phi), (-1.0, -phi)] {
        vertices.push(Vec3::new(b, 0.0, a).normalised());
    }

    // The faces are derived rather than tabulated. A hardcoded index table is only
    // correct for the vertex ordering it was written against, and pairing one with a
    // different ordering gives a shape that still looks like a ball and is silently
    // wrong everywhere — which is exactly what happened here first.
    //
    // On a unit icosahedron every adjacent pair of vertices has a dot product of
    // exactly 1/√5, and every non-adjacent pair has −1/√5 or −1. So adjacency is a
    // threshold, and the faces are the mutually adjacent triples.
    const ADJACENT: f64 = 0.447_213_595_499_958;
    let touches = |a: usize, b: usize| (vertices[a].dot(vertices[b]) - ADJACENT).abs() < 1e-6;

    let mut faces = Vec::with_capacity(20);
    for a in 0..12 {
        for b in (a + 1)..12 {
            if !touches(a, b) {
                continue;
            }
            for c in (b + 1)..12 {
                if touches(a, c) && touches(b, c) {
                    faces.push([a as u32, b as u32, c as u32]);
                }
            }
        }
    }
    debug_assert_eq!(faces.len(), 20, "an icosahedron has twenty faces");
    (vertices, faces)
}

fn subdivided_icosahedron(level: u8) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let (mut vertices, mut faces) = icosahedron();

    for _ in 0..level {
        // Midpoints are shared by two faces; the cache is what keeps the vertex count at
        // 10·4ⁿ+2 instead of tripling it with duplicates that would then fail to be
        // neighbours of anything.
        let mut midpoints: HashMap<(u32, u32), u32> = HashMap::new();
        let mut split = Vec::with_capacity(faces.len() * 4);

        for face in &faces {
            let [a, b, c] = *face;
            let mut midpoint = |p: u32, q: u32| -> u32 {
                let key = if p < q { (p, q) } else { (q, p) };
                if let Some(&existing) = midpoints.get(&key) {
                    return existing;
                }
                let point = vertices[p as usize].slerp_half(vertices[q as usize]);
                vertices.push(point);
                let index = vertices.len() as u32 - 1;
                midpoints.insert(key, index);
                index
            };
            let (ab, bc, ca) = (midpoint(a, b), midpoint(b, c), midpoint(c, a));
            split.push([a, ab, ca]);
            split.push([b, bc, ab]);
            split.push([c, ca, bc]);
            split.push([ab, bc, ca]);
        }
        faces = split;
    }

    (vertices, faces)
}

fn adjacency(count: usize, faces: &[[u32; 3]]) -> (Vec<CellId>, Vec<u8>) {
    let mut neighbours = vec![NONE; count * MAX_NEIGHBOURS];
    let mut degrees = vec![0u8; count];

    let link = |from: u32, to: u32, neighbours: &mut Vec<CellId>, degrees: &mut Vec<u8>| {
        let base = from as usize * MAX_NEIGHBOURS;
        let degree = degrees[from as usize] as usize;
        if neighbours[base..base + degree].contains(&to) {
            return;
        }
        debug_assert!(degree < MAX_NEIGHBOURS, "a geodesic cell has at most six");
        neighbours[base + degree] = to;
        degrees[from as usize] = degree as u8 + 1;
    };

    for face in faces {
        let [a, b, c] = *face;
        for (p, q) in [(a, b), (b, c), (c, a)] {
            link(p, q, &mut neighbours, &mut degrees);
            link(q, p, &mut neighbours, &mut degrees);
        }
    }
    (neighbours, degrees)
}

/// Each cell's territory: a third of every triangle it touches.
///
/// The dual cell around a vertex takes exactly one third of each incident triangle, so
/// this is the true area, not an estimate — and the totals add up to the sphere without
/// a normalisation fudge, which is what conservation laws downstream will lean on.
fn dual_areas(positions: &[Vec3], faces: &[[u32; 3]]) -> Vec<f64> {
    let mut areas = vec![0.0; positions.len()];
    for face in faces {
        let [a, b, c] = *face;
        let third = triangle_area(
            positions[a as usize],
            positions[b as usize],
            positions[c as usize],
        ) / 3.0;
        areas[a as usize] += third;
        areas[b as usize] += third;
        areas[c as usize] += third;
    }
    areas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_counts_follow_the_subdivision_formula() {
        for level in 0..=4u8 {
            let expected = 10 * 4usize.pow(level as u32) + 2;
            assert_eq!(Grid::new(level).len(), expected, "at level {level}");
        }
    }

    #[test]
    fn exactly_twelve_cells_are_pentagons() {
        // Euler's formula makes this unavoidable: you cannot tile a sphere with
        // hexagons alone. Worth pinning, because every field solver has to cope with
        // the exceptions and a bug that produced thirteen would be invisible otherwise.
        for level in 1..=4u8 {
            let grid = Grid::new(level);
            let fives = grid.cells().filter(|&c| grid.degree(c) == 5).count();
            let sixes = grid.cells().filter(|&c| grid.degree(c) == 6).count();
            assert_eq!(fives, 12, "at level {level}");
            assert_eq!(fives + sixes, grid.len(), "some cell had another degree");
        }
    }

    #[test]
    fn adjacency_is_symmetric() {
        let grid = Grid::new(3);
        for cell in grid.cells() {
            for &n in grid.neighbours(cell) {
                assert!(
                    grid.neighbours(n).contains(&cell),
                    "{cell} lists {n} but not the other way round"
                );
            }
            assert!(
                !grid.neighbours(cell).contains(&cell),
                "{cell} neighbours itself"
            );
        }
    }

    #[test]
    fn every_cell_sits_on_the_unit_sphere() {
        let grid = Grid::new(4);
        for cell in grid.cells() {
            assert!((grid.position(cell).length() - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn areas_add_up_to_a_sphere() {
        // If they did not, every conserved quantity in the climate would leak.
        for level in 0..=4u8 {
            let grid = Grid::new(level);
            let total: f64 = grid.cells().map(|c| grid.solid_angle(c)).sum();
            assert!(
                (total - 4.0 * std::f64::consts::PI).abs() < 1e-9,
                "level {level} summed to {total}"
            );
        }
    }

    #[test]
    fn cells_are_nearly_equal_in_area() {
        // The whole reason for a geodesic instead of lat/lon. Pentagons are the small
        // ones; nothing should be more than about a quarter off the mean.
        let grid = Grid::new(4);
        let mean = 4.0 * std::f64::consts::PI / grid.len() as f64;
        let (min, max) = grid.cells().fold((f64::MAX, 0.0f64), |(lo, hi), c| {
            let a = grid.solid_angle(c);
            (lo.min(a), hi.max(a))
        });
        assert!(
            min / mean > 0.75 && max / mean < 1.25,
            "areas ranged {:.3}× to {:.3}× the mean",
            min / mean,
            max / mean
        );
    }

    #[test]
    fn spacing_matches_the_documented_resolution() {
        // Level 6 is quoted as ~112 km on an Earth-sized planet, and that number is
        // used to justify what the physics can and cannot resolve. Checked at level 4
        // and scaled: spacing halves with each subdivision.
        let grid = Grid::new(4);
        let spacing = grid.spacing_km(6371.0);
        let at_six = spacing / 4.0;
        assert!(
            (at_six - 112.0).abs() < 15.0,
            "level 6 spacing would be {at_six:.0} km"
        );
    }

    #[test]
    fn the_nearest_cell_to_a_cell_is_itself() {
        let grid = Grid::new(3);
        for cell in grid.cells() {
            assert_eq!(grid.nearest_to(grid.position(cell), 0), cell);
        }
    }

    #[test]
    fn greedy_search_finds_the_true_nearest_from_anywhere() {
        // The hill climb is an optimisation, and an optimisation that quietly returns
        // the wrong cell would show up much later as plates leaking material. Swept
        // over the whole sphere rather than spot-checked, because the places it can go
        // wrong are the twelve pentagons and nowhere else.
        let grid = Grid::new(3);
        let mut probes = Vec::new();
        for i in 0..24 {
            for j in 0..24 {
                let z = -1.0 + 2.0 * (i as f64 + 0.5) / 24.0;
                let phi = std::f64::consts::TAU * (j as f64 + 0.5) / 24.0;
                let r = (1.0 - z * z).sqrt();
                probes.push(Vec3::new(r * phi.cos(), r * phi.sin(), z));
            }
        }
        for probe in probes {
            let brute = grid
                .cells()
                .max_by(|&a, &b| {
                    grid.position(a)
                        .dot(probe)
                        .total_cmp(&grid.position(b).dot(probe))
                })
                .unwrap();
            for hint in [0, 7, grid.len() as CellId - 1] {
                assert_eq!(grid.nearest_to(probe, hint), brute, "from hint {hint}");
            }
        }
    }

    #[test]
    fn neighbours_are_closer_than_non_neighbours() {
        let grid = Grid::new(3);
        let cell = 42;
        let far: f64 = grid
            .neighbours(cell)
            .iter()
            .map(|&n| grid.distance_km(cell, n, 6371.0))
            .fold(0.0, f64::max);
        let others = grid
            .cells()
            .filter(|c| *c != cell && !grid.neighbours(cell).contains(c));
        for other in others {
            assert!(grid.distance_km(cell, other, 6371.0) > far * 0.99);
        }
    }

    #[test]
    fn the_grid_is_one_connected_surface() {
        let grid = Grid::new(3);
        let mut seen = vec![false; grid.len()];
        let mut stack = vec![0 as CellId];
        seen[0] = true;
        while let Some(cell) = stack.pop() {
            for &n in grid.neighbours(cell) {
                if !seen[n as usize] {
                    seen[n as usize] = true;
                    stack.push(n);
                }
            }
        }
        assert!(seen.iter().all(|s| *s), "the sphere came apart");
    }
}
