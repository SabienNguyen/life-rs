//! Rendering a planet's deep history as a page you can scrub through.
//!
//! The lithosphere is forty thousand numbers changing over a billion years, which is
//! exactly the sort of thing that is unreadable as text and obvious as a picture. The
//! neighbourhood bug that emptied three quarters of a town was found by looking at a
//! chart rather than a log, and the same applies here with more force: nobody can tell
//! from a column of land fractions whether the continents are drifting sensibly.
//!
//! The page carries a handful of snapshots and interpolates nothing — each frame is a
//! real state of the planet, sampled onto an equirectangular grid at write time so the
//! viewer needs no geometry code at all.

use std::fmt::Write as _;

use biome::Biosphere;
use climate::Climate;
use ecology::Ecology;
use geo::{Boundary, CellId, Lithosphere};

/// Pixels across the map. Equirectangular, so half as many down.
///
/// Sized against the page rather than against the grid: every frame carries three bytes
/// a pixel and the whole history has to arrive in one file, so doubling this quadruples
/// what somebody has to load before they see anything. Two hundred and forty across is
/// about a hundred and seventy kilometres a pixel at the equator, which resolves a
/// level-five grid comfortably and a level-six one well enough to see the continents.
const WIDE: usize = 240;
const TALL: usize = WIDE / 2;

/// One state of the planet, flattened to a picture and a row of numbers.
pub struct Frame {
    pub myr: f64,
    /// Elevation relative to sea level, in metres, row-major from the north pole.
    pub height: Vec<i16>,
    /// Biome and boundary kind, packed one byte per pixel: the biome in the top four
    /// bits, the boundary in the bottom two. Fifteen biomes and four boundary kinds fit
    /// in six bits between them, and a byte a pixel is a byte a pixel — this used to
    /// carry the plate number instead, which was of far less interest than what grows
    /// there.
    pub tenure: Vec<u8>,
    /// Temperature in half-degrees from −64 °C, and rainfall in units of twenty
    /// millimetres a year. One byte each: the map cannot draw finer than that and the
    /// whole history has to arrive in one file.
    pub temperature: Vec<u8>,
    pub rain: Vec<u8>,
    /// How many species live in each pixel.
    pub richness: Vec<u8>,
    pub sea_level_m: f32,
    pub land_fraction: f32,
    pub continental_fraction: f32,
    pub plates: usize,
    pub largest_landmass: f32,
    pub peak_m: f32,
    pub mean_temp_c: f32,
    pub co2_ppm: f32,
    pub ice_fraction: f32,
    pub temperate_fraction: f32,
    pub mean_rain_mm: f32,
    pub forest_share: f32,
    pub desert_share: f32,
    pub production_gt: f32,
    pub species: usize,
    pub animal_mt: f32,
    pub extinctions: usize,
}

/// Sample the planet onto an equirectangular grid.
///
/// Walks the pixels in scanline order and hands each lookup the previous pixel's answer
/// as its starting point. Neighbouring pixels are neighbouring places, so the search
/// almost always finishes in a hop or two — which is what makes sampling a hundred
/// thousand points per frame cost nothing worth measuring.
pub fn sample(planet: &Lithosphere, climate: &Climate, life: &Biosphere, fauna: &Ecology) -> Frame {
    let mut height = Vec::with_capacity(WIDE * TALL);
    let mut tenure = Vec::with_capacity(WIDE * TALL);
    let mut temperature = Vec::with_capacity(WIDE * TALL);
    let mut rain = Vec::with_capacity(WIDE * TALL);
    let mut richness = Vec::with_capacity(WIDE * TALL);
    let mut hint: CellId = 0;

    for row in 0..TALL {
        let latitude = 90.0 - 180.0 * (row as f64 + 0.5) / TALL as f64;
        for column in 0..WIDE {
            let longitude = -180.0 + 360.0 * (column as f64 + 0.5) / WIDE as f64;
            let (lat, lon) = (latitude.to_radians(), longitude.to_radians());
            let direction = geo::Vec3::new(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin());
            let cell = planet.grid().nearest_to(direction, hint);
            hint = cell;
            height.push(planet.height_above_sea_m(cell).clamp(-11_000.0, 11_000.0) as i16);
            let kind = match planet.boundary(cell) {
                Boundary::Interior => 0u8,
                Boundary::Divergent => 1,
                Boundary::Convergent => 2,
                Boundary::Transform => 3,
            };
            tenure.push((life.biome(cell) as u8) << 4 | kind);
            temperature.push(((climate.temperature_c(cell) + 64.0) * 2.0).clamp(0.0, 255.0) as u8);
            rain.push((climate.rain_mm(cell) / 20.0).clamp(0.0, 255.0) as u8);
            richness.push(fauna.richness_at(cell).min(255) as u8);
        }
    }

    Frame {
        myr: planet.age_myr(),
        height,
        tenure,
        temperature,
        rain,
        richness,
        sea_level_m: planet.sea_level_m(),
        land_fraction: planet.land_fraction(),
        continental_fraction: planet.continental_fraction(),
        plates: planet.active_plates(),
        largest_landmass: planet.largest_landmass_share(),
        peak_m: planet
            .grid()
            .cells()
            .map(|c| planet.height_above_sea_m(c))
            .fold(f32::MIN, f32::max),
        mean_temp_c: climate.mean_temperature_c(planet),
        co2_ppm: climate.co2_ppm(),
        ice_fraction: climate.ice_fraction(planet),
        temperate_fraction: climate.temperate_fraction(planet),
        mean_rain_mm: climate.mean_rain_mm(planet),
        forest_share: life.forest_share(planet),
        desert_share: life.desert_share(planet),
        production_gt: life.total_production_gt(planet),
        species: fauna.richness(),
        animal_mt: fauna.total_biomass_mt(),
        extinctions: fauna.lost,
    }
}

/// Fill the viewer template with a run's frames.
pub fn page(template: &str, seed: &str, level: u8, frames: &[Frame]) -> String {
    template
        .replace("__GLOBE_DATA__", &data(seed, level, frames))
        .replace("__GLOBE_WIDE__", &WIDE.to_string())
        .replace("__GLOBE_TALL__", &TALL.to_string())
}

fn data(seed: &str, level: u8, frames: &[Frame]) -> String {
    let mut out = String::from("{\n");
    let _ = write!(out, "  \"seed\": \"{seed}\",\n  \"level\": {level},\n");
    let _ = write!(out, "  \"wide\": {WIDE},\n  \"tall\": {TALL},\n");
    out.push_str("  \"frames\": [");

    for (i, frame) in frames.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        // The maps go out as base64 rather than as arrays of numbers: fifty thousand
        // pixels written as JSON text is roughly six times the bytes, per frame, and
        // this file is meant to be opened rather than downloaded.
        let _ = write!(
            out,
            "\n    {{\"myr\":{:.0},\"sea\":{:.0},\"land\":{:.4},\"crust\":{:.4},\
             \"plates\":{},\"biggest\":{:.3},\"peak\":{:.0},\"temp\":{:.2},\
             \"co2\":{:.0},\"ice\":{:.4},\"temperate\":{:.4},\"rainfall\":{:.0},\
             \"forest\":{:.4},\"arid\":{:.4},\"biomass\":{:.1},\
             \"species\":{},\"animals\":{:.1},\"extinct\":{},\
             \"height\":\"{}\",\"tenure\":\"{}\",\"warmth\":\"{}\",\"wet\":\"{}\",\"kinds\":\"{}\"}}",
            frame.myr,
            frame.sea_level_m,
            frame.land_fraction,
            frame.continental_fraction,
            frame.plates,
            frame.largest_landmass,
            frame.peak_m,
            frame.mean_temp_c,
            frame.co2_ppm,
            frame.ice_fraction,
            frame.temperate_fraction,
            frame.mean_rain_mm,
            frame.forest_share,
            frame.desert_share,
            frame.production_gt,
            frame.species,
            frame.animal_mt,
            frame.extinctions,
            base64(&as_bytes(&frame.height)),
            base64(&frame.tenure),
            base64(&frame.temperature),
            base64(&frame.rain),
            base64(&frame.richness),
        );
    }
    out.push_str("\n  ]\n}");
    out
}

/// Little-endian pairs, which is what a `DataView` in the page will read back.
fn as_bytes(values: &[i16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 2);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64, hand-rolled for the same reason the JSON is.
fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let packed = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(packed >> 18) as usize & 63] as char);
        out.push(ALPHABET[(packed >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(packed >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[packed as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Domain, WorldSeed};

    fn a_world() -> (Lithosphere, Climate, Biosphere, Ecology) {
        let seed = WorldSeed::from_u128(0x5eed);
        let mut rng = seed.stream(Domain::Terrain, 0, 0);
        let mut planet = Lithosphere::genesis(4, 9, 0.42, &mut rng);
        planet.step_myr(2.0, &mut rng);
        let climate = Climate::genesis(&planet, 4.57, climate::insolation::EARTH_OBLIQUITY);
        let life = Biosphere::read(&planet, &climate);
        let mut fauna = Ecology::genesis(&planet, &life, &climate, 24, seed);
        for _ in 0..4 {
            fauna.step_myr(&planet, &life, &climate, 1.0, &mut rng);
        }
        (planet, climate, life, fauna)
    }

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        // The high bytes matter: elevations go out as signed pairs.
        assert_eq!(base64(&[0xff, 0xfe, 0xfd]), "//79");
    }

    #[test]
    fn sixteen_bit_values_survive_the_round_trip() {
        let values: Vec<i16> = vec![0, -1, 1, -11_000, 11_000, i16::MIN, i16::MAX];
        let bytes = as_bytes(&values);
        assert_eq!(bytes.len(), values.len() * 2);
        let back: Vec<i16> = bytes
            .chunks(2)
            .map(|p| i16::from_le_bytes([p[0], p[1]]))
            .collect();
        assert_eq!(back, values);
    }

    #[test]
    fn a_frame_covers_the_whole_map() {
        let (planet, climate, life, fauna) = a_world();
        let frame = sample(&planet, &climate, &life, &fauna);
        assert_eq!(frame.height.len(), WIDE * TALL);
        assert_eq!(frame.tenure.len(), WIDE * TALL);
        assert_eq!(frame.temperature.len(), WIDE * TALL);
        assert_eq!(frame.rain.len(), WIDE * TALL);
        assert!(frame.height.iter().any(|h| *h > 0), "no land anywhere");
        assert!(frame.height.iter().any(|h| *h < 0), "no sea anywhere");
        // Every pixel names a real biome, packed into the top four bits.
        for byte in &frame.tenure {
            assert!(
                (byte >> 4) < biome::Biome::COUNT as u8,
                "a pixel claimed biome {}",
                byte >> 4
            );
        }
        // Temperature is stored offset and doubled; the tropics must come back warm.
        let warmest = *frame.temperature.iter().max().unwrap();
        assert!(
            warmest as f32 / 2.0 - 64.0 > 10.0,
            "the warmest place on the planet read {:.0} °C",
            warmest as f32 / 2.0 - 64.0
        );
        assert!(
            frame.rain.iter().any(|r| *r > 0),
            "it never rained anywhere"
        );
    }

    #[test]
    fn the_map_wraps_and_the_poles_are_poles() {
        // An equirectangular map is a cylinder: the last column has to be the same
        // place as the first. A seam here means every pixel is offset by half a cell.
        let (planet, climate, life, fauna) = a_world();
        let frame = sample(&planet, &climate, &life, &fauna);
        let row = TALL / 2;
        let west = frame.height[row * WIDE];
        let east = frame.height[row * WIDE + WIDE - 1];
        assert!(
            (west - east).abs() < 3000,
            "the map has a seam: {west} m against {east} m"
        );

        // And the top row is one place, not a stretched line of different ones.
        let top: Vec<u8> = (0..WIDE).map(|c| frame.tenure[c] >> 4).collect();
        let distinct = top.iter().collect::<std::collections::BTreeSet<_>>().len();
        assert!(distinct <= 3, "the north pole spanned {distinct} biomes");
    }

    #[test]
    fn the_page_has_no_placeholders_left() {
        let (planet, climate, life, fauna) = a_world();
        let frame = sample(&planet, &climate, &life, &fauna);
        let filled = page(
            "<b>__GLOBE_DATA__</b> __GLOBE_WIDE__x__GLOBE_TALL__",
            "0x1",
            4,
            &[frame],
        );
        assert!(!filled.contains("__GLOBE"), "a placeholder survived");
        assert!(filled.contains("\"frames\""));
        assert!(filled.contains(&format!("{WIDE}x{TALL}")));
    }
}
