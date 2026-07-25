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
    /// Plate and boundary kind, packed one byte per pixel: the plate in the top six
    /// bits, the boundary in the bottom two. Two fields that are only ever read
    /// together, and halving the bytes they cost is worth the shift.
    pub tenure: Vec<u8>,
    pub sea_level_m: f32,
    pub land_fraction: f32,
    pub continental_fraction: f32,
    pub plates: usize,
    pub largest_landmass: f32,
    pub peak_m: f32,
}

/// Sample the planet onto an equirectangular grid.
///
/// Walks the pixels in scanline order and hands each lookup the previous pixel's answer
/// as its starting point. Neighbouring pixels are neighbouring places, so the search
/// almost always finishes in a hop or two — which is what makes sampling a hundred
/// thousand points per frame cost nothing worth measuring.
pub fn sample(planet: &Lithosphere) -> Frame {
    let mut height = Vec::with_capacity(WIDE * TALL);
    let mut tenure = Vec::with_capacity(WIDE * TALL);
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
            tenure.push(((planet.plate_of(cell) % 64) as u8) << 2 | kind);
        }
    }

    Frame {
        myr: planet.age_myr(),
        height,
        tenure,
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
             \"plates\":{},\"biggest\":{:.3},\"peak\":{:.0},\
             \"height\":\"{}\",\"tenure\":\"{}\"}}",
            frame.myr,
            frame.sea_level_m,
            frame.land_fraction,
            frame.continental_fraction,
            frame.plates,
            frame.largest_landmass,
            frame.peak_m,
            base64(&as_bytes(&frame.height)),
            base64(&frame.tenure),
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

    fn a_planet() -> Lithosphere {
        let mut rng = WorldSeed::from_u128(0x5eed).stream(Domain::Terrain, 0, 0);
        Lithosphere::genesis(4, 9, 0.42, &mut rng)
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
        let frame = sample(&a_planet());
        assert_eq!(frame.height.len(), WIDE * TALL);
        assert_eq!(frame.tenure.len(), WIDE * TALL);
        assert!(frame.height.iter().any(|h| *h > 0), "no land anywhere");
        assert!(frame.height.iter().any(|h| *h < 0), "no sea anywhere");
    }

    #[test]
    fn the_map_wraps_and_the_poles_are_poles() {
        // An equirectangular map is a cylinder: the last column has to be the same
        // place as the first. A seam here means every pixel is offset by half a cell.
        let frame = sample(&a_planet());
        let row = TALL / 2;
        let west = frame.height[row * WIDE];
        let east = frame.height[row * WIDE + WIDE - 1];
        assert!(
            (west - east).abs() < 3000,
            "the map has a seam: {west} m against {east} m"
        );

        // And the top row is one place, not a stretched line of different ones.
        let top: Vec<u8> = (0..WIDE).map(|c| frame.tenure[c] >> 2).collect();
        let distinct = top.iter().collect::<std::collections::BTreeSet<_>>().len();
        assert!(distinct <= 3, "the north pole spanned {distinct} plates");
    }

    #[test]
    fn the_page_has_no_placeholders_left() {
        let frame = sample(&a_planet());
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
