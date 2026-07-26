//! Where people live on the planet, and why there and not somewhere else.
//!
//! This is the join. Below it the stack runs plates → climate → biomes → animals →
//! evolution, a megayear at a time, and has never had a person on it. Above it people are
//! born, pair, raise children and sort themselves between neighbourhoods, and have never
//! stood anywhere in particular. The two halves ran eleven rungs of the time ladder apart
//! and the only thing missing between them was an answer to *where*.
//!
//! ## The projection
//!
//! Everything here is a projection downwards. The planet knows elevation, crustal
//! thickness, sediment depth, sea level, temperature, rainfall, ice cover, net primary
//! production and which of fifteen biomes a cell is. A human life turns on almost none of
//! that. It turns on whether the land feeds you, whether anyone can reach you, how hard
//! the year is, and how many of you the place will hold — so that is what comes across,
//! and the rest stays on the planet's side of the join where it belongs.
//!
//! The alternative — handing the society crate a grid and a climate and letting it read
//! whatever it liked — is the shape that makes a simulation impossible to reason about,
//! because then every rule about neighbourhoods is also a rule about geophysics.
//!
//! ## Choosing sites
//!
//! Habitability is a product rather than a sum, and that is the whole of the model:
//! **a place has to be survivable, and fed, and reachable, and all three at once**. Sums
//! let a spectacular score on one term carry a zero on another, which puts cities in the
//! middle of ice caps because the fishing is good offshore. Products do not.
//!
//! Sites are then picked greedily from the best cells with a minimum separation, because
//! two settlements in adjacent cells are one settlement. The separation is what makes the
//! chosen set look like a map of somewhere rather than a heat map's brightest pixels.

use biome::{Biome, Biosphere};
use climate::Climate;
use geo::{CellId, Lithosphere};
use sim_core::Rng;
use society::Terrain;

pub mod naming;

/// The temperature a place is easiest to live at, in °C.
///
/// Not the temperature people like — the temperature at which the year costs least. The
/// densest parts of the real planet sit within a few degrees of this, and they did so
/// long before anybody could heat a building.
const COMFORTABLE_C: f32 = 15.0;
/// How far from comfortable the climate can get before the place is uninhabitable, in °C.
///
/// Generous, because people demonstrably live in Yakutsk and in Death Valley. It is the
/// width of the tolerable band, not of the pleasant one.
const TOLERANCE_C: f32 = 32.0;
/// Rain below which the land will not feed anyone without a river, in mm/yr.
const PARCHED_MM: f32 = 120.0;
/// The scale on which land productivity is felt, in g/m²/yr.
///
/// Fertility saturates against this rather than clamping at it, and the difference
/// mattered: with a hard ceiling at what a temperate forest makes, every decent site on
/// the planet came out at exactly one and the term stopped distinguishing anything. A
/// saturating curve keeps the shape that is actually true — the step from bare rock to
/// thin pasture is worth far more than the step from good land to better — without ever
/// running out of range.
const AMPLE_NPP: f32 = 900.0;
/// Elevation above which thin air starts to cost, in metres.
const THIN_AIR_M: f32 = 2200.0;
/// Elevation at which almost nobody lives, in metres.
const TOO_HIGH_M: f32 = 5000.0;

/// How many households a square kilometre of thoroughly fertile land will carry.
///
/// A pre-industrial ceiling: roughly forty people to the square kilometre of good farmed
/// land, at five to a household. Rich river valleys beat it by an order of magnitude and
/// this model does not know about rivers, so it is a floor on what the real number would
/// be — which is the right direction to be wrong in, because the alternative is a world
/// whose carrying capacity is never once the binding constraint on anything.
const HOUSEHOLDS_PER_KM2: f64 = 8.0;

/// One place people could live, with the reason it is one.
#[derive(Clone, Debug)]
pub struct Site {
    pub name: String,
    pub terrain: Terrain,
    /// How good this is to live in, 0 to 1 — the product of the four terms below.
    pub habitability: f32,
}

/// How habitable every cell of the planet is, and why.
///
/// Kept as a whole map rather than computed per candidate because the reach term needs to
/// look at neighbours, and because having the map is what lets the viewer draw it.
pub struct Habitability {
    score: Vec<f32>,
    fertility: Vec<f32>,
    reach: Vec<f32>,
    harshness: Vec<f32>,
}

impl Habitability {
    /// Read the planet.
    pub fn of(planet: &Lithosphere, climate: &Climate, life: &Biosphere) -> Habitability {
        let grid = planet.grid();
        let n = grid.len();
        let mut fertility = vec![0.0f32; n];
        let mut harshness = vec![1.0f32; n];

        for cell in grid.cells() {
            if !planet.is_land(cell) {
                continue;
            }
            fertility[cell as usize] = 1.0 - (-life.production(cell) / AMPLE_NPP).exp();
            harshness[cell as usize] = harshness_of(planet, climate, cell);
        }

        // Reach needs the neighbourhood, so it comes second. A coast is reachable because
        // the sea is a road — which is true for almost all of history and stops being
        // true only with railways, so it is the right default for a world without them.
        let mut reach = vec![0.0f32; n];
        for cell in grid.cells() {
            if !planet.is_land(cell) {
                continue;
            }
            let neighbours = grid.neighbours(cell);
            let coastal = neighbours.iter().any(|&n| !planet.is_land(n));
            // Ground you can cross. Not the cell's own height — the *difference* between
            // it and what surrounds it, because a high plateau is easy going and a low
            // valley between two ranges is not.
            let here = planet.height_above_sea_m(cell);
            let ruggedness = neighbours
                .iter()
                .map(|&n| (planet.height_above_sea_m(n) - here).abs())
                .fold(0.0f32, f32::max);
            let passable = (1.0 - ruggedness / 3000.0).clamp(0.0, 1.0);
            // Somewhere to go once you have set out.
            let company = neighbours.iter().filter(|&&n| planet.is_land(n)).count() as f32
                / neighbours.len() as f32;

            reach[cell as usize] =
                (0.15 + 0.40 * f32::from(coastal) + 0.25 * passable + 0.20 * company)
                    .clamp(0.0, 1.0);
        }

        // Survivability, fed, reachable — multiplied, so a zero anywhere is a zero.
        let score = (0..n)
            .map(|i| {
                let cell = i as CellId;
                if !planet.is_land(cell) {
                    return 0.0;
                }
                let survivable = 1.0 - harshness[i];
                // Fed is not linear in fertility: the difference between bare rock and
                // thin pasture matters far more than between good land and better.
                let fed = fertility[i].sqrt();
                survivable * fed * reach[i]
            })
            .collect();

        Habitability {
            score,
            fertility,
            reach,
            harshness,
        }
    }

    pub fn score(&self, cell: CellId) -> f32 {
        self.score[cell as usize]
    }

    pub fn fertility(&self, cell: CellId) -> f32 {
        self.fertility[cell as usize]
    }

    pub fn reach(&self, cell: CellId) -> f32 {
        self.reach[cell as usize]
    }

    pub fn harshness(&self, cell: CellId) -> f32 {
        self.harshness[cell as usize]
    }

    /// The share of the planet's surface anyone could live on at all.
    pub fn habitable_fraction(&self, planet: &Lithosphere) -> f32 {
        let grid = planet.grid();
        let mut lived = 0.0;
        let mut total = 0.0;
        for cell in grid.cells() {
            let area = grid.solid_angle(cell);
            total += area;
            if self.score(cell) > 0.05 {
                lived += area;
            }
        }
        if total <= 0.0 {
            0.0
        } else {
            (lived / total) as f32
        }
    }
}

/// How hard a cell is to live in, 0 to 1, before anything about food.
fn harshness_of(planet: &Lithosphere, climate: &Climate, cell: CellId) -> f32 {
    if climate.is_frozen(cell) {
        return 1.0;
    }
    let from_comfort = (climate.temperature_c(cell) - COMFORTABLE_C).abs() / TOLERANCE_C;
    let thirst = if climate.rain_mm(cell) >= PARCHED_MM {
        0.0
    } else {
        1.0 - climate.rain_mm(cell) / PARCHED_MM
    };
    let height = planet.height_above_sea_m(cell);
    let altitude = ((height - THIN_AIR_M) / (TOO_HIGH_M - THIN_AIR_M)).clamp(0.0, 1.0);
    // The worst of the three rather than their sum: what makes a place unlivable is
    // whichever thing about it is unlivable, and adding them lets three mild
    // inconveniences add up to a place nobody could survive.
    from_comfort.max(thirst).max(altitude).clamp(0.0, 1.0)
}

/// The single best place on the planet to be, which is where a people starts.
///
/// The jitter is what makes two worlds with identical continents settle them differently.
/// It is a tenth of the range, which reorders near-ties and cannot promote a glacier over
/// a floodplain.
pub fn heartland(planet: &Lithosphere, habitability: &Habitability, rng: &mut Rng) -> Option<CellId> {
    let mut best: Option<(CellId, f32)> = None;
    for cell in planet.grid().cells() {
        let score = habitability.score(cell);
        if score <= 0.02 {
            continue;
        }
        let jittered = score * (0.9 + 0.2 * rng.unit_f32());
        if best.is_none_or(|(_, b)| jittered > b) {
            best = Some((cell, jittered));
        }
    }
    best.map(|(cell, _)| cell)
}

/// Pick where a founding population would settle, around a heartland.
///
/// The first version of this took the best cells on the planet outright, and drawing the
/// result was enough to see what was wrong with it: the five quarters of one town came
/// out on three different continents, at 128° east, 75° west and 165° east. They are
/// neighbourhoods of one society, not five unrelated civilisations — so the search is
/// bounded to the country around the best cell, and what varies between them is the
/// difference between the good ground at the centre and the poorer ground at the edges.
/// Which is what a region is.
///
/// Greedy from the best cell down within that bound, refusing anything within `apart`
/// rings of a site already taken.
pub fn sites(
    planet: &Lithosphere,
    habitability: &Habitability,
    centre: CellId,
    reach_rings: usize,
    wanted: usize,
    apart: usize,
    rng: &mut Rng,
) -> Vec<Site> {
    let grid = planet.grid();

    let mut ranked: Vec<(CellId, f32)> = neighbourhood(grid, centre, reach_rings)
        .into_iter()
        .filter(|&c| habitability.score(c) > 0.01)
        .map(|c| (c, habitability.score(c) * (0.9 + 0.2 * rng.unit_f32())))
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));

    let mut taken: Vec<CellId> = Vec::new();
    let mut sites = Vec::new();
    for (cell, _) in ranked {
        if sites.len() >= wanted {
            break;
        }
        if taken.iter().any(|&t| within(grid, t, cell, apart)) {
            continue;
        }
        taken.push(cell);
        sites.push(site_at(planet, habitability, cell, rng));
    }
    sites
}

/// Every cell within `rings` steps of one, including it.
fn neighbourhood(grid: &geo::Grid, centre: CellId, rings: usize) -> Vec<CellId> {
    let mut seen = vec![centre];
    let mut frontier = vec![centre];
    for _ in 0..rings {
        let mut next = Vec::new();
        for cell in frontier.drain(..) {
            for &n in grid.neighbours(cell) {
                if !seen.contains(&n) {
                    seen.push(n);
                    next.push(n);
                }
            }
        }
        frontier = next;
    }
    seen
}

/// Whether two cells are within `rings` steps of each other on the grid.
fn within(grid: &geo::Grid, a: CellId, b: CellId, rings: usize) -> bool {
    if a == b {
        return true;
    }
    let mut frontier = vec![a];
    let mut seen = vec![a];
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

/// Describe one cell as a place people could live.
fn site_at(
    planet: &Lithosphere,
    habitability: &Habitability,
    cell: CellId,
    rng: &mut Rng,
) -> Site {
    let grid = planet.grid();
    let position = grid.position(cell);
    let area_km2 = grid.area_km2(cell, geo::EARTH_RADIUS_KM);
    let fertility = habitability.fertility(cell);
    let reach = habitability.reach(cell);
    let harshness = habitability.harshness(cell);

    // What the cell will carry. Fertility is the whole of it — how reachable somewhere is
    // changes who wants to be there, not how many the ground feeds.
    let carrying = (area_km2 * HOUSEHOLDS_PER_KM2 * fertility as f64).round();
    let coastal = grid.neighbours(cell).iter().any(|&n| !planet.is_land(n));

    let terrain = Terrain {
        cell,
        latitude: position.latitude().to_degrees() as f32,
        longitude: position.longitude().to_degrees() as f32,
        elevation_m: planet.height_above_sea_m(cell),
        fertility,
        reach,
        harshness,
        // Clamped above one, so a marginal site is somewhere small rather than somewhere
        // impossible — the greedy pass has already refused anything genuinely dead.
        carrying: (carrying as u32).max(1),
        biome: "",
    };

    Site {
        name: naming::name_for(&terrain, coastal, rng),
        terrain,
        habitability: habitability.score(cell),
    }
}

/// Fill in the biome label, which needs the biosphere the terrain does not carry.
pub fn label(site: &mut Site, life: &Biosphere) {
    site.terrain.biome = life.biome(site.terrain.cell).label();
}

/// How far a region reaches, in rings of the grid.
///
/// Two, which at the grids this is used on is a country rather than a county — a coarse
/// grid buys the planet's plate motion at a price paid here, and the price is that the
/// smallest thing a settlement can be is very large. It is the right number regardless:
/// what it has to be is far enough that neighbouring quarters differ, near enough that
/// they are the same society.
pub const REGION_RINGS: usize = 2;

/// Everything a settled world needs to know about the ground under it.
///
/// Returns empty for a planet with nowhere habitable on it, which is a planet this can
/// legitimately produce and which callers have to handle rather than assume away.
pub fn survey(
    planet: &Lithosphere,
    climate: &Climate,
    life: &Biosphere,
    wanted: usize,
    apart: usize,
    rng: &mut Rng,
) -> Vec<Site> {
    let habitability = Habitability::of(planet, climate, life);
    let Some(centre) = heartland(planet, &habitability, rng) else {
        return Vec::new();
    };
    // Widen the search until there is room for everyone asked for. A region on a narrow
    // peninsula holds fewer quarters than one on a plain, and the honest response to that
    // is to spread out rather than to stack settlements on top of each other.
    let mut sites = Vec::new();
    for rings in REGION_RINGS..=REGION_RINGS + 4 {
        sites = self::sites(planet, &habitability, centre, rings, wanted, apart, rng);
        if sites.len() >= wanted {
            break;
        }
    }
    for site in &mut sites {
        label(site, life);
    }
    sites
}

/// Whether a biome is one people could plausibly farm.
pub fn is_farmable(biome: Biome) -> bool {
    matches!(
        biome,
        Biome::Grassland
            | Biome::TemperateForest
            | Biome::TemperateRainforest
            | Biome::Shrubland
            | Biome::Savanna
            | Biome::SeasonalForest
            | Biome::Rainforest
    )
}

#[cfg(test)]
mod tests;
