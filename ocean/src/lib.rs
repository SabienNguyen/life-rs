//! The sea, and why some of it is alive.
//!
//! Two thirds of the planet, and until now it was a temperature and a depth. What was
//! missing is the thing that actually decides where the ocean is productive, and it is
//! not light and it is not warmth — the sunlit tropical ocean is one of the emptiest
//! places on Earth. It is **nutrients**, and nutrients are in the deep water. Everything
//! that lives in the sunlit layer sinks when it dies, so the surface is stripped and the
//! depths are rich, and the only places with both light and nutrients are the places
//! where deep water is being brought up.
//!
//! So this crate is really about one quantity — upwelling — and the circulation exists to
//! produce it.
//!
//! ## What is modelled
//!
//! **Wind-driven gyres.** The wind belts are a function of latitude and nothing else at
//! this resolution: easterlies in the tropics, westerlies in the mid-latitudes, easterlies
//! at the poles. Wind stress curl over a basin spins a gyre; the gyre's sense follows the
//! hemisphere. This is Sverdrup balance with the arithmetic taken out, which is defensible
//! precisely because the *pattern* is robust — every ocean basin on Earth has the same
//! gyres in the same places, because the wind belts put them there.
//!
//! **Coastal upwelling.** Where a wind blows along a coast with the land on the left in
//! the northern hemisphere (right in the southern), Ekman transport pushes surface water
//! offshore and deep water rises to replace it. This is why the great fisheries are on the
//! *eastern* sides of oceans — Peru, California, Benguela, Canary — and it is one of the
//! few pieces of physical oceanography whose consequences you can point at on a map.
//!
//! **Equatorial divergence.** The Coriolis parameter changes sign at the equator, so the
//! trade winds push surface water away from it in both directions and deep water rises
//! along the line. The Pacific cold tongue is this.
//!
//! **Overturning.** Cold, salty water at high latitudes sinks; it has to come up
//! somewhere, and it comes up diffusely everywhere. One bulk term, driven by how cold the
//! coldest water is, because that is what sets the rate.
//!
//! ## What is not
//!
//! Salinity, oxygen, a real Sverdrup transport, western boundary currents as distinct
//! features, and any of it evolving in time. There is no Gulf Stream here, which means no
//! Europe-is-warmer-than-Labrador and no shutdown-of-the-overturning event. The heat
//! transport in `climate` is a diffusivity that is already higher over water, which stands
//! in for all of it — crudely, and knowingly.

use climate::Climate;
use geo::{CellId, Lithosphere};

pub mod nutrients;

/// The strength of the trade winds and the westerlies, in arbitrary units where one is a
/// typical open-ocean wind.
const TRADE_STRENGTH: f32 = 1.0;
const WESTERLY_STRENGTH: f32 = 1.15;
const POLAR_STRENGTH: f32 = 0.55;

/// Latitude where the trades give way to the westerlies, in degrees.
const HORSE_LATITUDE: f64 = 30.0;
/// Latitude where the westerlies give way to the polar easterlies.
const POLAR_FRONT: f64 = 60.0;

/// How strong equatorial divergence is, in the same units upwelling is reported in.
const EQUATORIAL_LIFT: f32 = 1.0;
/// How far from the equator that divergence reaches, in degrees.
const EQUATORIAL_REACH: f64 = 6.0;
/// How strong coastal upwelling is where the geometry is exactly right.
const COASTAL_LIFT: f32 = 1.4;
/// The diffuse return flow of the overturning circulation, everywhere at once.
const OVERTURNING_FLOOR: f32 = 0.10;

/// Which way the wind blows, as an along-parallel component and a poleward one.
///
/// Eastward positive. `poleward` is **hemisphere-relative**: positive is towards whichever
/// pole this hemisphere has, so the wind field is a mirror image across the equator and
/// says so. Converting it to a true northward component is one multiplication and it
/// belongs at the one place that needs a true direction, which is the Ekman rotation.
/// Baking the hemisphere in here instead was the first version, and it made the field
/// silently asymmetric — the trades came out blowing towards the equator in the north and
/// away from it in the south.
///
/// Reduced to two numbers per cell because the only thing downstream asks of the wind is
/// what it does to the water under it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wind {
    pub eastward: f32,
    pub poleward: f32,
}

/// The wind belt at a latitude.
///
/// A function of latitude alone, which is what makes it cheap and what makes it a model
/// rather than a simulation. It is also very nearly true: the belts are set by the Hadley,
/// Ferrel and polar cells, and those are set by how much sun falls where.
pub fn wind_at(latitude_deg: f64) -> Wind {
    let from_equator = latitude_deg.abs();
    if from_equator < HORSE_LATITUDE {
        // Trades: out of the east, and towards the equator.
        Wind {
            eastward: -TRADE_STRENGTH,
            poleward: -0.35 * TRADE_STRENGTH,
        }
    } else if from_equator < POLAR_FRONT {
        // Westerlies: out of the west, and towards the pole.
        Wind {
            eastward: WESTERLY_STRENGTH,
            poleward: 0.30 * WESTERLY_STRENGTH,
        }
    } else {
        Wind {
            eastward: -POLAR_STRENGTH,
            poleward: -0.20 * POLAR_STRENGTH,
        }
    }
}

/// The state of the sea, cell by cell.
pub struct Ocean {
    upwelling: Vec<f32>,
    nutrients: Vec<f32>,
    wind: Vec<Wind>,
    /// Which basin each cell belongs to, or `NO_BASIN` on land.
    basin: Vec<u16>,
    basins: usize,
}

/// The basin id land carries.
pub const NO_BASIN: u16 = u16::MAX;

impl Ocean {
    /// Read the sea off a planet and its climate.
    ///
    /// Stateless, like the biosphere and for the same reason: everything here is a
    /// function of where the continents are and how cold the water is, and both of those
    /// are already stored somewhere else. Storing the answer as well is a second copy that
    /// can disagree with the first.
    pub fn read(planet: &Lithosphere, climate: &Climate) -> Ocean {
        let grid = planet.grid();
        let n = grid.len();

        let mut wind = Vec::with_capacity(n);
        for cell in grid.cells() {
            wind.push(wind_at(grid.position(cell).latitude().to_degrees()));
        }

        let (basin, basins) = basins_of(planet);
        let mut upwelling = vec![0.0f32; n];

        // The coldest surface water anywhere sets the overturning rate: the circulation is
        // driven by water dense enough to sink, and nothing else about it matters at this
        // resolution. A planet with no cold water anywhere has a sluggish ocean, which is
        // what the warmest periods in the record look like.
        let coldest = grid
            .cells()
            .filter(|&c| !planet.is_land(c))
            .map(|c| climate.temperature_c(c))
            .fold(f32::MAX, f32::min);
        let overturning = OVERTURNING_FLOOR * (1.0 - (coldest + 4.0) / 24.0).clamp(0.2, 1.4);

        for cell in grid.cells() {
            if planet.is_land(cell) {
                continue;
            }
            let latitude = grid.position(cell).latitude().to_degrees();
            let mut lift = overturning;

            // Equatorial divergence: the Coriolis parameter changes sign here, so the
            // trades push water away from the line in both directions and deep water fills
            // the gap. The Pacific cold tongue is exactly this.
            let from_equator = latitude.abs();
            if from_equator < EQUATORIAL_REACH {
                lift += EQUATORIAL_LIFT * (1.0 - from_equator / EQUATORIAL_REACH) as f32;
            }

            lift += coastal_lift(planet, grid, cell, latitude, wind[cell as usize]);
            upwelling[cell as usize] = lift;
        }

        let nutrients = nutrients::from_upwelling(planet, climate, &upwelling);

        Ocean {
            upwelling,
            nutrients,
            wind,
            basin,
            basins,
        }
    }

    /// How much deep water reaches the surface here, in units where one is a strong
    /// coastal upwelling zone and zero is a dead calm.
    pub fn upwelling(&self, cell: CellId) -> f32 {
        self.upwelling[cell as usize]
    }

    /// How much of what the sunlit layer needs is available, 0 to 1.
    pub fn nutrients(&self, cell: CellId) -> f32 {
        self.nutrients[cell as usize]
    }

    pub fn wind(&self, cell: CellId) -> Wind {
        self.wind[cell as usize]
    }

    /// Which body of water this cell belongs to. `NO_BASIN` on land.
    pub fn basin(&self, cell: CellId) -> u16 {
        self.basin[cell as usize]
    }

    /// How many separate seas the planet has.
    pub fn basins(&self) -> usize {
        self.basins
    }

    /// The share of the sea that is fed well enough to be worth fishing.
    pub fn fertile_share(&self, planet: &Lithosphere) -> f32 {
        let grid = planet.grid();
        let mut fed = 0.0;
        let mut sea = 0.0;
        for cell in grid.cells() {
            if planet.is_land(cell) {
                continue;
            }
            let area = grid.solid_angle(cell);
            sea += area;
            if self.nutrients(cell) > 0.5 {
                fed += area;
            }
        }
        if sea <= 0.0 { 0.0 } else { (fed / sea) as f32 }
    }

    /// Mean upwelling over the whole sea, for watching it change.
    pub fn mean_upwelling(&self, planet: &Lithosphere) -> f32 {
        let grid = planet.grid();
        let mut total = 0.0;
        let mut sea = 0.0;
        for cell in grid.cells() {
            if planet.is_land(cell) {
                continue;
            }
            let area = grid.solid_angle(cell);
            sea += area;
            total += area * self.upwelling(cell) as f64;
        }
        if sea <= 0.0 { 0.0 } else { (total / sea) as f32 }
    }
}

/// Where the wind actually pushes the surface layer, as (eastward, northward).
///
/// Ekman's result: integrate the wind-driven flow over the depth the wind reaches and the
/// net transport is ninety degrees off the wind — to the right in the northern hemisphere,
/// to the left in the southern. It is one of the few pieces of geophysical fluid dynamics
/// that reduces to a rotation, and it is the reason the world's great fisheries are where
/// they are.
///
/// Note what happens on an eastern shore in the trade belt. The rotation reverses across
/// the equator *and so does the meridional wind*, so the two reversals cancel and the
/// offshore transport comes out the same sign in both hemispheres. That is why Peru and
/// California are both upwelling coasts despite being mirror images.
pub fn ekman_transport(wind: Wind, latitude_deg: f64) -> (f32, f32) {
    let hemisphere = if latitude_deg >= 0.0 { 1.0f32 } else { -1.0 };
    let northward = wind.poleward * hemisphere;
    if latitude_deg >= 0.0 {
        // Right of the wind: (e, n) → (n, −e).
        (northward, -wind.eastward)
    } else {
        // Left of the wind: (e, n) → (−n, e).
        (-northward, wind.eastward)
    }
}

/// Coastal upwelling at one cell.
///
/// The rule is Ekman's and it is the reason the world's great fisheries are where they
/// are. Surface water driven by wind is deflected ninety degrees — right in the northern
/// hemisphere, left in the southern. Where that deflection points *away* from a coast, the
/// surface layer is pulled offshore and deep water rises to replace it. Where it points
/// into the coast, water piles up and the opposite happens.
///
/// On the real planet this puts the upwelling on the **eastern** side of ocean basins,
/// because there the equatorward trades and the land combine the right way round: Peru,
/// California, Benguela, the Canaries. Nothing here knows the word "eastern" — it falls
/// out of the geometry, which is the whole point of deriving it.
fn coastal_lift(
    planet: &Lithosphere,
    grid: &geo::Grid,
    cell: CellId,
    latitude_deg: f64,
    wind: Wind,
) -> f32 {
    // Which way the land is, as a direction on the sphere from here to the mean of the
    // land neighbours. No land next door is no coast and no coastal upwelling.
    let mut toward_land = geo::Vec3::new(0.0, 0.0, 0.0);
    let mut coastal = false;
    let here = grid.position(cell);
    for &n in grid.neighbours(cell) {
        if planet.is_land(n) {
            coastal = true;
            toward_land = toward_land.plus(grid.position(n).minus(here));
        }
    }
    if !coastal || toward_land.length() < 1e-9 {
        return 0.0;
    }
    let toward_land = toward_land.normalised();

    // A local frame: east along the parallel, north along the meridian.
    let north_pole = geo::Vec3::new(0.0, 0.0, 1.0);
    let east = north_pole.cross(here);
    if east.length() < 1e-6 {
        return 0.0; // Directly over a pole; there is no east there.
    }
    let east = east.normalised();
    let north = here.cross(east).normalised();

    let (transport_east, transport_north) = ekman_transport(wind, latitude_deg);
    let transport = east
        .scaled(transport_east as f64)
        .plus(north.scaled(transport_north as f64));

    // Offshore transport is transport pointing away from the land.
    let offshore = -transport.dot(toward_land) as f32;
    if offshore <= 0.0 {
        return 0.0;
    }
    // The Coriolis parameter vanishes at the equator, so there is no Ekman transport there
    // however hard the wind blows — which is why coastal upwelling is a mid-latitude and
    // subtropical phenomenon and the equatorial kind has a different cause.
    let coriolis = latitude_deg.to_radians().sin().abs() as f32;
    COASTAL_LIFT * offshore * coriolis
}

/// Label each connected body of water, so a landlocked sea is not the same place as an
/// ocean.
///
/// It matters for more than bookkeeping: a small enclosed basin behaves nothing like an
/// open ocean, and knowing which is which is what a later salinity or oxygen model would
/// be built on. For now it is what the observer needs to say "an inland sea" rather than
/// "water".
fn basins_of(planet: &Lithosphere) -> (Vec<u16>, usize) {
    let grid = planet.grid();
    let mut basin = vec![NO_BASIN; grid.len()];
    let mut count = 0u16;
    let mut stack = Vec::new();

    for start in grid.cells() {
        if planet.is_land(start) || basin[start as usize] != NO_BASIN {
            continue;
        }
        // Saturating, so a pathological planet with sixty-five thousand ponds labels the
        // last of them as land rather than wrapping to basin zero.
        if count == NO_BASIN {
            break;
        }
        let id = count;
        count += 1;
        basin[start as usize] = id;
        stack.push(start);
        while let Some(cell) = stack.pop() {
            for &n in grid.neighbours(cell) {
                if !planet.is_land(n) && basin[n as usize] == NO_BASIN {
                    basin[n as usize] = id;
                    stack.push(n);
                }
            }
        }
    }
    (basin, count as usize)
}

#[cfg(test)]
mod tests;
