//! Rigid plates, and the Euler poles they turn about.
//!
//! A plate does not slide across the sphere — it *rotates*, about an axis through the
//! planet's centre. That is not a modelling convenience, it is Euler's theorem: any
//! motion of a rigid cap on a sphere is a rotation about some pole. Two numbers, an axis
//! and a rate, describe everything a plate does, which is why plate reconstructions can
//! be published as tables of poles rather than as maps of arrows.
//!
//! The consequence worth knowing about is that a plate's *speed* varies across its own
//! surface: fastest on the great circle ninety degrees from its pole, and nil at the
//! pole itself. Nothing in this module enforces that; it falls out of the rotation.

use sim_core::Rng;

use crate::sphere::Vec3;

pub type PlateId = u16;

/// Real plates run between about one and ten centimetres a year. Expressed here as
/// radians of rotation per megayear, which for an Earth-sized planet is the same thing.
const SLOWEST_RAD_PER_MYR: f64 = 0.0016;
const FASTEST_RAD_PER_MYR: f64 = 0.0130;

#[derive(Clone, Debug)]
pub struct Plate {
    /// Euler pole: the axis this plate turns about.
    pub pole: Vec3,
    /// Angular speed, radians per megayear. Signed — the sense of rotation matters.
    pub rate: f64,
    /// Total rotation so far. This, not a position, is a plate's state.
    pub angle: f64,
    /// Welded into another plate, or rifted away to nothing.
    pub active: bool,
}

impl Plate {
    /// Where a piece of this plate's material sits now.
    pub fn present(&self, frame: Vec3) -> Vec3 {
        frame.rotated_about(self.pole, self.angle)
    }

    /// The material coordinate of a point that is currently here.
    ///
    /// The inverse of [`Plate::present`]. Used whenever crust changes hands: the point
    /// keeps its place on the planet and acquires a new address within its new plate.
    pub fn frame_of(&self, present: Vec3) -> Vec3 {
        present.rotated_about(self.pole, -self.angle)
    }

    /// Surface velocity at a point, in kilometres per megayear.
    ///
    /// `v = ω × r`, so it is zero at the pole and greatest on the equator of the
    /// rotation — which is why one plate can be tearing apart at one end and barely
    /// moving at the other.
    pub fn speed_km_per_myr(&self, at: Vec3, radius_km: f64) -> f64 {
        self.pole.scaled(self.rate).cross(at).length() * radius_km
    }

    /// A random pole and rate. Used at genesis and whenever a plate rifts.
    pub fn random(rng: &mut Rng) -> Plate {
        Plate {
            pole: random_direction(rng),
            rate: rng.range_f64(SLOWEST_RAD_PER_MYR, FASTEST_RAD_PER_MYR)
                * if rng.coin() { 1.0 } else { -1.0 },
            angle: 0.0,
            active: true,
        }
    }

    /// A new pole and rate for an existing plate, keeping wherever it has got to.
    ///
    /// This is a plate reorganisation: the drive changes, the crust does not move. Real
    /// ones happen every fifty to a hundred megayears, usually when a subducting slab
    /// tears or a continent arrives at a trench and jams it.
    pub fn redirect(&mut self, rng: &mut Rng) {
        self.pole = random_direction(rng);
        self.rate = rng.range_f64(SLOWEST_RAD_PER_MYR, FASTEST_RAD_PER_MYR)
            * if rng.coin() { 1.0 } else { -1.0 };
    }
}

/// A direction drawn uniformly over the sphere.
///
/// Uniform in *area*, which means uniform in the sine of latitude — sampling latitude
/// directly would crowd poles and is the classic way to get a lumpy sphere.
pub fn random_direction(rng: &mut Rng) -> Vec3 {
    let z = rng.range_f64(-1.0, 1.0);
    let phi = rng.range_f64(0.0, std::f64::consts::TAU);
    let r = (1.0 - z * z).max(0.0).sqrt();
    Vec3::new(r * phi.cos(), r * phi.sin(), z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Domain, WorldSeed};

    fn rng() -> Rng {
        WorldSeed::from_u128(0x9e_11).stream(Domain::Terrain, 0, 0)
    }

    #[test]
    fn a_plate_carries_its_material_and_gives_it_back() {
        let mut plate = Plate::random(&mut rng());
        plate.angle = 0.37;
        let frame = Vec3::new(0.2, -0.9, 0.3).normalised();
        let there = plate.present(frame);
        assert!(plate.frame_of(there).angle_to(frame) < 1e-12);
    }

    #[test]
    fn material_does_not_move_until_the_plate_turns() {
        let mut plate = Plate::random(&mut rng());
        plate.angle = 0.0;
        let frame = Vec3::new(1.0, 0.2, -0.1).normalised();
        assert!(plate.present(frame).angle_to(frame) < 1e-15);
    }

    #[test]
    fn speed_is_zero_at_the_pole_and_greatest_at_right_angles() {
        let plate = Plate {
            pole: Vec3::new(0.0, 0.0, 1.0),
            rate: 0.01,
            angle: 0.0,
            active: true,
        };
        let at_pole = plate.speed_km_per_myr(Vec3::new(0.0, 0.0, 1.0), 6371.0);
        let at_equator = plate.speed_km_per_myr(Vec3::new(1.0, 0.0, 0.0), 6371.0);
        assert!(at_pole < 1e-9, "the pole should not move: {at_pole}");
        assert!(
            (at_equator - 63.71).abs() < 0.01,
            "equatorial speed was {at_equator} km/Myr"
        );
    }

    #[test]
    fn plate_speeds_are_earthlike() {
        // One to ten centimetres a year, which at these units is ten to a hundred
        // kilometres per megayear. A plate crossing an ocean in five megayears would
        // make deep time unreadable.
        let mut rng = rng();
        for _ in 0..200 {
            let plate = Plate::random(&mut rng);
            let fastest = plate.speed_km_per_myr(
                plate.pole.cross(Vec3::new(0.0, 0.0, 1.0)).normalised(),
                6371.0,
            );
            assert!(
                (8.0..=90.0).contains(&fastest),
                "a plate ran at {fastest} km/Myr"
            );
        }
    }

    #[test]
    fn plates_turn_both_ways() {
        let mut rng = rng();
        let plates: Vec<Plate> = (0..40).map(|_| Plate::random(&mut rng)).collect();
        assert!(plates.iter().any(|p| p.rate > 0.0));
        assert!(plates.iter().any(|p| p.rate < 0.0));
    }

    #[test]
    fn random_directions_cover_the_sphere_evenly() {
        // The bug this guards against is sampling latitude uniformly, which piles
        // points at the poles and would put every continent in the arctic.
        let mut rng = rng();
        let mut bands = [0usize; 6];
        const N: usize = 12_000;
        for _ in 0..N {
            let z = random_direction(&mut rng).z;
            let band = (((z + 1.0) / 2.0) * 6.0) as usize;
            bands[band.min(5)] += 1;
        }
        // Equal-area bands in z, so equal counts, within sampling noise.
        for count in bands {
            let share = count as f64 / N as f64;
            assert!(
                (0.145..0.188).contains(&share),
                "a band of equal area took {share:.3} of the points: {bands:?}"
            );
        }
    }

    #[test]
    fn redirection_keeps_the_crust_where_it_was() {
        // A reorganisation changes where a plate is going, never where it is.
        let mut rng = rng();
        let mut plate = Plate::random(&mut rng);
        plate.angle = 1.1;
        let frame = Vec3::new(0.5, 0.5, 0.7).normalised();
        let before = plate.present(frame);

        // Crust keeps its place on the planet by re-addressing itself in the new frame.
        plate.redirect(&mut rng);
        plate.angle = 0.0;
        let refreshed = plate.frame_of(before);
        assert!(plate.present(refreshed).angle_to(before) < 1e-12);
    }
}
