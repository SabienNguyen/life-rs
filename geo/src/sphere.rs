//! Points and rotations on a unit sphere.
//!
//! Three-component vectors and Rodrigues rotation, written out rather than pulled in.
//! The whole of it is forty lines, it is used in exactly one crate, and a linear algebra
//! dependency would arrive with a hundred things this simulation will never ask for.

/// A direction from the centre of the planet. Kept normalised by construction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3 { x, y, z }
    }

    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn normalised(self) -> Vec3 {
        let len = self.length();
        if len == 0.0 {
            // Degenerate input has to go somewhere; the pole is as good as anywhere and
            // beats propagating NaN through an entire planet.
            return Vec3::new(0.0, 0.0, 1.0);
        }
        Vec3::new(self.x / len, self.y / len, self.z / len)
    }

    pub fn scaled(self, k: f64) -> Vec3 {
        Vec3::new(self.x * k, self.y * k, self.z * k)
    }

    pub fn plus(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    pub fn minus(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    /// Midpoint of two directions, projected back onto the sphere.
    pub fn slerp_half(self, other: Vec3) -> Vec3 {
        self.plus(other).normalised()
    }

    /// Angle between two directions, in radians. This is great-circle distance on a
    /// unit sphere; multiply by the planet's radius for kilometres.
    pub fn angle_to(self, other: Vec3) -> f64 {
        // Via atan2 of the cross and dot rather than acos of the dot: acos loses all
        // its precision for nearby points, which is exactly the case that matters when
        // measuring the distance between neighbouring cells.
        self.cross(other).length().atan2(self.dot(other))
    }

    /// Latitude in radians, positive north.
    pub fn latitude(self) -> f64 {
        self.z.clamp(-1.0, 1.0).asin()
    }

    /// Longitude in radians.
    pub fn longitude(self) -> f64 {
        self.y.atan2(self.x)
    }

    /// Rotate about an axis by an angle, right-handed. Rodrigues' formula.
    ///
    /// This is how a plate moves: an Euler pole is an axis, an angular rate times a
    /// span is an angle, and everything welded to the plate turns with it.
    pub fn rotated_about(self, axis: Vec3, radians: f64) -> Vec3 {
        let axis = axis.normalised();
        let (sin, cos) = radians.sin_cos();
        self.scaled(cos)
            .plus(axis.cross(self).scaled(sin))
            .plus(axis.scaled(axis.dot(self) * (1.0 - cos)))
            .normalised()
    }
}

/// Area of a spherical triangle in steradians, by L'Huilier's theorem.
///
/// The naive spherical-excess formula subtracts three angles each near π/3 and keeps
/// the remainder, which for the small triangles of a level-6 grid is catastrophic
/// cancellation. L'Huilier works in half-tangents of the *sides* and stays accurate all
/// the way down.
pub fn triangle_area(a: Vec3, b: Vec3, c: Vec3) -> f64 {
    let (sa, sb, sc) = (b.angle_to(c), c.angle_to(a), a.angle_to(b));
    let s = 0.5 * (sa + sb + sc);
    let t =
        (s / 2.0).tan() * ((s - sa) / 2.0).tan() * ((s - sb) / 2.0).tan() * ((s - sc) / 2.0).tan();
    4.0 * t.max(0.0).sqrt().atan()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f64 = 1e-12;

    #[test]
    fn rotation_preserves_length_and_the_axis() {
        let axis = Vec3::new(0.3, -0.7, 0.5).normalised();
        let point = Vec3::new(1.0, 2.0, -0.5).normalised();
        let turned = point.rotated_about(axis, 1.234);
        assert!((turned.length() - 1.0).abs() < EPS);
        // A rotation cannot change how far a point is from its own axis.
        assert!((turned.dot(axis) - point.dot(axis)).abs() < 1e-9);
    }

    #[test]
    fn a_full_turn_comes_back() {
        let axis = Vec3::new(0.0, 0.0, 1.0);
        let point = Vec3::new(1.0, 0.0, 0.0);
        let turned = point.rotated_about(axis, std::f64::consts::TAU);
        assert!(turned.angle_to(point) < 1e-9);
    }

    #[test]
    fn rotation_is_reversible() {
        // Plate motion is un-run constantly: to ask what material sits under a point,
        // the point is carried backwards into the plate's own frame.
        let axis = Vec3::new(1.0, 1.0, 0.2).normalised();
        let point = Vec3::new(-0.4, 0.9, 0.1).normalised();
        let there = point.rotated_about(axis, 0.7);
        let back = there.rotated_about(axis, -0.7);
        assert!(back.angle_to(point) < 1e-12);
    }

    #[test]
    fn an_octant_is_an_eighth_of_the_sphere() {
        let area = triangle_area(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );
        assert!((area - std::f64::consts::PI / 2.0).abs() < 1e-12);
    }

    #[test]
    fn tiny_triangles_do_not_collapse_to_zero() {
        // The case that breaks the textbook formula. At level 6 every triangle is this
        // small, so getting nonsense here would mean getting nonsense everywhere.
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 1e-4, 0.0).normalised();
        let c = Vec3::new(1.0, 0.0, 1e-4).normalised();
        let area = triangle_area(a, b, c);
        // A nearly-flat right triangle with legs 1e-4: area ≈ half the product.
        assert!(
            (area - 0.5e-8).abs() < 1e-11,
            "area came out {area}, expected about 5e-9"
        );
    }

    #[test]
    fn a_degenerate_triangle_has_no_area() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        assert!(triangle_area(a, b, a).abs() < EPS);
    }

    #[test]
    fn angles_are_accurate_for_close_points() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 1e-9, 0.0).normalised();
        assert!((a.angle_to(b) - 1e-9).abs() < 1e-18);
    }

    #[test]
    fn latitude_and_longitude_read_off_the_vector() {
        let north = Vec3::new(0.0, 0.0, 1.0);
        assert!((north.latitude() - std::f64::consts::FRAC_PI_2).abs() < EPS);
        let east = Vec3::new(0.0, 1.0, 0.0);
        assert!((east.longitude() - std::f64::consts::FRAC_PI_2).abs() < EPS);
        assert!(east.latitude().abs() < EPS);
    }
}
