//! The one piece of geometry the climate needs that the planet does not already have.

use geo::Vec3;

/// A unit vector at `from`, lying flat on the sphere, pointing towards `to`.
///
/// The direction you would set off in to walk there. Used to ask which neighbours lie
/// downwind, which is a question about direction along the surface rather than about
/// the straight line through the rock between two points.
pub fn tangent_towards(from: Vec3, to: Vec3) -> Vec3 {
    // Remove whatever part of the difference points into or out of the planet.
    let along = to.minus(from.scaled(from.dot(to)));
    if along.length() < 1e-12 {
        // The same point, or its antipode: no direction is the right answer, so give a
        // consistent one rather than a NaN.
        return Vec3::new(0.0, 0.0, 0.0);
    }
    along.normalised()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_points_along_the_surface_and_not_through_it() {
        let from = Vec3::new(1.0, 0.0, 0.0);
        let to = Vec3::new(0.0, 1.0, 0.0);
        let heading = tangent_towards(from, to);
        assert!(heading.dot(from).abs() < 1e-12, "it left the surface");
        assert!((heading.length() - 1.0).abs() < 1e-12);
        assert!(heading.dot(to) > 0.99, "it pointed the wrong way");
    }

    #[test]
    fn heading_north_from_the_equator_is_north() {
        let from = Vec3::new(1.0, 0.0, 0.0);
        let to = Vec3::new(0.9, 0.0, 0.43).normalised();
        let heading = tangent_towards(from, to);
        assert!(heading.z > 0.99, "north came out as {heading:?}");
    }

    #[test]
    fn a_point_and_itself_has_no_direction() {
        let here = Vec3::new(0.3, -0.5, 0.8).normalised();
        assert_eq!(tangent_towards(here, here).length(), 0.0);
        assert_eq!(tangent_towards(here, here.scaled(-1.0)).length(), 0.0);
    }

    #[test]
    fn opposite_neighbours_give_opposite_headings() {
        let here = Vec3::new(0.0, 0.0, 1.0);
        let east = tangent_towards(here, Vec3::new(0.2, 0.0, 0.98).normalised());
        let west = tangent_towards(here, Vec3::new(-0.2, 0.0, 0.98).normalised());
        assert!(east.dot(west) < -0.99, "{east:?} against {west:?}");
    }
}
