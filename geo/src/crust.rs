//! Crust, isostasy, and where the sea comes to rest.
//!
//! Elevation is never stored as an authored number. It is read off crust thickness and
//! density by Airy isostasy — the crust floats on the mantle, and how high it rides is a
//! question of how thick and how light it is. That single rule is why continents stand
//! above oceans, why a collision that doubles crustal thickness makes a plateau, and why
//! erosion is slow to win: strip a kilometre off a mountain and isostatic rebound gives
//! most of it back.
//!
//! Oceanic crust is the exception, and for a real reason: fresh ocean floor is hot, and
//! its elevation is governed by how far it has cooled rather than by its thickness. The
//! √age subsidence law below is the half-space cooling solution, and it is one of the
//! best-verified relationships in the earth sciences.

/// Densities in kg/m³.
const MANTLE: f32 = 3300.0;
const CONTINENTAL: f32 = 2800.0;
const OCEANIC: f32 = 2900.0;
const SEDIMENT: f32 = 2200.0;

/// Unstretched continental crust, in kilometres. Thicker makes mountains, thinner makes
/// shelves and rift basins.
pub const CONTINENTAL_THICKNESS_KM: f32 = 35.0;
/// Fresh ocean floor, in kilometres. Remarkably uniform on the real planet.
pub const OCEANIC_THICKNESS_KM: f32 = 7.0;

/// How much of a kilometre of continental crust ends up above the mantle line.
pub const BUOYANCY: f32 = 1.0 - CONTINENTAL / MANTLE;
const OCEANIC_BUOYANCY: f32 = 1.0 - OCEANIC / MANTLE;
const SEDIMENT_BUOYANCY: f32 = 1.0 - SEDIMENT / MANTLE;

/// Where zero is: the height of unstretched continental crust.
///
/// Not sea level, and deliberately not. Sea level is an *output* — it depends on how
/// much water there is and what shape the basins are — so pinning the datum to it would
/// make every elevation in the model move whenever the sea did. Anchoring instead to a
/// fixed thickness of crust gives a stable reference; [`Lithosphere::height_above_sea_m`]
/// is what to ask when the question is about the shoreline.
const DATUM_KM: f32 = CONTINENTAL_THICKNESS_KM * BUOYANCY;

/// Ridge crest depth in metres, and the subsidence coefficient in metres per √Myr.
const RIDGE_DEPTH_M: f32 = 2600.0;
const SUBSIDENCE_M_PER_ROOT_MYR: f32 = 345.0;
/// Old ocean floor stops sinking: the plate reaches a steady thickness and the
/// half-space solution over-predicts past roughly 80 Myr.
const ABYSSAL_FLOOR_M: f32 = 5800.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrustType {
    Continental,
    Oceanic,
}

impl CrustType {
    pub const fn label(self) -> &'static str {
        match self {
            CrustType::Continental => "continental",
            CrustType::Oceanic => "oceanic",
        }
    }

    pub const fn is_oceanic(self) -> bool {
        matches!(self, CrustType::Oceanic)
    }
}

/// Surface elevation in metres above the datum, before erosion.
pub fn elevation_m(crust: CrustType, thickness_km: f32, age_myr: f32, sediment_m: f32) -> f32 {
    let bedrock = match crust {
        CrustType::Continental => (thickness_km * BUOYANCY - DATUM_KM) * 1000.0,
        CrustType::Oceanic => {
            // Cooling sets the depth; any crust beyond the standard seven kilometres —
            // an oceanic plateau, a hotspot pile — floats it back up.
            let cooled = (RIDGE_DEPTH_M + SUBSIDENCE_M_PER_ROOT_MYR * age_myr.max(0.0).sqrt())
                .min(ABYSSAL_FLOOR_M);
            -cooled + (thickness_km - OCEANIC_THICKNESS_KM) * 1000.0 * OCEANIC_BUOYANCY
        }
    };
    bedrock + sediment_m * SEDIMENT_BUOYANCY
}

/// How much crust must be removed to lower the surface by one metre.
///
/// Around six and a half. This is why mountains are stubborn: erosion has to shift six
/// metres of rock to win one metre of height, and the other five come back up.
pub fn isostatic_amplification() -> f32 {
    1.0 / BUOYANCY
}

/// Where the sea comes to rest, given a fixed volume of water and the shape of the land.
///
/// Sea level is not a constant here. Water fills whatever basins exist, so when
/// continents aggregate and the ocean basins change shape, the shoreline moves on its
/// own — flooding continental interiors in some configurations and draining shelves in
/// others. Bisection on the filled volume, which is monotonic in the level and so cannot
/// have more than one answer.
pub fn sea_level_m(elevations: &[f32], areas_km2: &[f64], water_km3: f64) -> f32 {
    debug_assert_eq!(elevations.len(), areas_km2.len());
    let filled = |level: f64| -> f64 {
        elevations
            .iter()
            .zip(areas_km2)
            .filter(|(e, _)| (**e as f64) < level)
            .map(|(e, a)| (level - *e as f64) * a / 1000.0)
            .sum()
    };

    let mut low = elevations.iter().copied().fold(f32::MAX, f32::min) as f64;
    let mut high = elevations.iter().copied().fold(f32::MIN, f32::max) as f64;
    if filled(high) < water_km3 {
        // More water than the basins can hold: a waterworld. Honest answer rather than
        // a silent clamp.
        return high as f32;
    }
    // Forty halvings of a fifteen-kilometre range resolves the level to a fraction of
    // a micrometre, which is far past what the crust it is standing on is known to.
    for _ in 0..40 {
        let mid = 0.5 * (low + high);
        if filled(mid) < water_km3 {
            low = mid;
        } else {
            high = mid;
        }
    }
    (0.5 * (low + high)) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_crust_defines_the_datum() {
        let h = elevation_m(CrustType::Continental, CONTINENTAL_THICKNESS_KM, 0.0, 0.0);
        assert!(h.abs() < 0.5, "reference crust came out at {h} m, not zero");
    }

    #[test]
    fn thickened_crust_builds_a_plateau() {
        // The Tibet test. Sixty-five kilometres of crust — nearly double — should stand
        // four and a half kilometres over ordinary continent, which is what the real
        // one does.
        let h = elevation_m(CrustType::Continental, 65.0, 0.0, 0.0);
        assert!(
            (4000.0..5000.0).contains(&h),
            "thickened crust came out at {h} m above the datum"
        );
    }

    #[test]
    fn stretched_crust_drops_below_the_datum() {
        // Thin the crust and the surface falls, without anything needing to decide that
        // a shelf should be there. Where the water then stands is a separate question.
        let shelf = elevation_m(CrustType::Continental, 25.0, 0.0, 0.0);
        assert!(
            (-2000.0..-1000.0).contains(&shelf),
            "stretched crust came out at {shelf} m"
        );
    }

    #[test]
    fn ocean_floor_deepens_with_age() {
        let ridge = elevation_m(CrustType::Oceanic, OCEANIC_THICKNESS_KM, 0.0, 0.0);
        let mature = elevation_m(CrustType::Oceanic, OCEANIC_THICKNESS_KM, 60.0, 0.0);
        let ancient = elevation_m(CrustType::Oceanic, OCEANIC_THICKNESS_KM, 200.0, 0.0);

        assert!((ridge + 2600.0).abs() < 1.0, "ridge crest at {ridge} m");
        assert!(
            (-5400.0..-4700.0).contains(&mature),
            "60 Myr floor at {mature} m"
        );
        assert!(ancient < mature, "older floor must be deeper");
        assert!(ancient > -6000.0, "and the plate model flattens it out");
    }

    #[test]
    fn subsidence_follows_the_root_of_age_not_age() {
        // Straight-line subsidence would sink the old Pacific to fifteen kilometres.
        let at = |age| -elevation_m(CrustType::Oceanic, OCEANIC_THICKNESS_KM, age, 0.0);
        let first = at(25.0) - at(0.0);
        let later = at(100.0) - at(75.0);
        assert!(
            first > later * 1.5,
            "the first 25 Myr should sink much faster: {first} then {later}"
        );
    }

    #[test]
    fn continents_stand_kilometres_above_the_sea_floor() {
        let land = elevation_m(CrustType::Continental, CONTINENTAL_THICKNESS_KM, 0.0, 0.0);
        let floor = elevation_m(CrustType::Oceanic, OCEANIC_THICKNESS_KM, 60.0, 0.0);
        let step = land - floor;
        assert!(
            (4500.0..6000.0).contains(&step),
            "the continental freeboard came out at {step} m"
        );
    }

    #[test]
    fn sediment_raises_a_basin_but_not_by_its_own_thickness() {
        let bare = elevation_m(CrustType::Oceanic, OCEANIC_THICKNESS_KM, 100.0, 0.0);
        let filled = elevation_m(CrustType::Oceanic, OCEANIC_THICKNESS_KM, 100.0, 3000.0);
        let rise = filled - bare;
        assert!(
            (900.0..1100.0).contains(&rise),
            "three kilometres of sediment lifted the floor {rise} m"
        );
    }

    #[test]
    fn erosion_has_to_move_six_metres_to_win_one() {
        let factor = isostatic_amplification();
        assert!((factor - 6.6).abs() < 0.1, "amplification was {factor}");
    }

    #[test]
    fn earthlike_crust_and_earths_water_leave_about_a_third_of_it_dry() {
        // The calibration that matters, and it is a calibration of the *outputs*: give
        // the model Earth's proportion of continental crust, Earth's spread of sea-floor
        // ages, and Earth's water, and roughly Earth's share of the planet should end up
        // above the waterline. Nothing sets the land fraction; it is where the sea comes
        // to rest against the crust that happens to exist.
        let cells = 1000;
        let continental = 400;
        let mut elevations = Vec::new();
        for i in 0..cells {
            elevations.push(if i < continental {
                // Cratons through to stretched margins, which is what makes shelves.
                let thickness = 24.0 + 18.0 * (i as f32 / continental as f32);
                elevation_m(CrustType::Continental, thickness, 0.0, 0.0)
            } else {
                // Sea floor spans nothing to about 150 Myr and no older, because older
                // than that has been subducted.
                let age = 150.0 * ((i - continental) as f32 / (cells - continental) as f32);
                elevation_m(CrustType::Oceanic, OCEANIC_THICKNESS_KM, age, 0.0)
            });
        }
        let area = 4.0 * std::f64::consts::PI * 6371.0 * 6371.0 / cells as f64;
        let areas = vec![area; cells];
        let level = sea_level_m(&elevations, &areas, 1.335e9);

        let dry = elevations.iter().filter(|e| **e > level).count() as f32 / cells as f32;
        assert!(
            (0.20..0.40).contains(&dry),
            "an Earth-like planet came out {dry:.2} dry, with the sea at {level:.0} m"
        );
        let mean_land: f32 = elevations
            .iter()
            .filter(|e| **e > level)
            .map(|e| *e - level)
            .sum::<f32>()
            / (dry * cells as f32);
        assert!(
            (200.0..2000.0).contains(&mean_land),
            "mean land stood {mean_land:.0} m above the sea"
        );
    }

    #[test]
    fn deeper_basins_draw_the_sea_down() {
        // The mechanism behind sea-level change over deep time: the same water in a
        // roomier ocean stands lower, and floods the continents when it does not.
        let areas = vec![1.0e6; 100];
        let shallow: Vec<f32> = (0..100)
            .map(|i| if i < 70 { -2000.0 } else { 500.0 })
            .collect();
        let deep: Vec<f32> = (0..100)
            .map(|i| if i < 70 { -5000.0 } else { 500.0 })
            .collect();
        let water = 1.0e8;
        assert!(sea_level_m(&deep, &areas, water) < sea_level_m(&shallow, &areas, water));
    }

    #[test]
    fn more_water_than_basins_gives_a_waterworld_rather_than_nonsense() {
        let areas = vec![1.0e6; 10];
        let elevations = vec![100.0; 10];
        let level = sea_level_m(&elevations, &areas, 1.0e12);
        assert!(level.is_finite());
        assert!(level >= 100.0);
    }
}
