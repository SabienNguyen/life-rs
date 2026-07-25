//! How much sunlight reaches each latitude, and how that changes over deep time.
//!
//! Computed rather than fitted. The usual shortcut in an energy-balance model is to
//! write the annual-mean insolation as `1 + S₂·P₂(sin φ)` and look up `S₂ = −0.48` for
//! Earth — but that constant is only Earth's, and obliquity is one of the three orbital
//! parameters this world is supposed to be able to vary. Integrating the daily-mean
//! insolation over a year is a dozen lines and costs nothing at the scales that use it,
//! and it gets the extreme cases right: at zero obliquity the poles receive almost
//! nothing, and past 54° they receive more over a year than the equator does.

use std::f64::consts::PI;

/// The solar constant today, in watts per square metre.
pub const SOLAR_CONSTANT: f64 = 1361.0;
/// Earth's obliquity, in degrees.
pub const EARTH_OBLIQUITY: f64 = 23.44;

/// How far the tilt wanders either side of its mean, in degrees, and how long a cycle
/// takes, in megayears.
///
/// Earth's runs between about 22.1° and 24.5° on a 41,000-year period. Two and a half
/// degrees sounds like nothing and is not: it is tens of watts per square metre at the
/// latitudes where ice sheets live, and it is the pacemaker of the glacial cycles.
const TILT_SWING_DEG: f64 = 1.2;
const TILT_PERIOD_MYR: f64 = 0.041;

/// The tilt at a given moment, given the mean it wanders about.
///
/// At megayear steps this is aliased into nonsense — forty-one thousand years is a
/// fortieth of a step — and that is the honest situation rather than a bug: a model that
/// strides across a cycle cannot resolve it. Callers stepping in megayears should pass
/// the mean and get the mean. It is here for the kiloyear stepping that glacial cycles
/// need, and so that the machinery exists before anything asks for it.
pub fn obliquity_at(age_gyr: f64, mean_deg: f64) -> f64 {
    let turns = age_gyr * 1000.0 / TILT_PERIOD_MYR;
    mean_deg + TILT_SWING_DEG * (turns * std::f64::consts::TAU).sin()
}

/// How bright the sun is at a given age of the system, relative to today.
///
/// The standard main-sequence brightening: a star fusing hydrogen leaves behind a
/// denser, hotter core. Four billion years ago the sun was about three quarters as
/// bright, which is the faint young sun problem — a planet with today's atmosphere would
/// have been frozen solid, and was not. What resolves it here is the same carbon cycle
/// that resolves it on the real planet.
pub fn brightness_at(age_gyr: f64) -> f64 {
    const NOW_GYR: f64 = 4.57;
    1.0 / (1.0 + 0.4 * (1.0 - age_gyr / NOW_GYR))
}

/// Annual-mean insolation at a latitude, in watts per square metre.
///
/// `latitude` in radians, `obliquity` in degrees.
pub fn annual_mean(latitude: f64, obliquity: f64, solar_constant: f64) -> f64 {
    // Sampling the year rather than integrating in closed form. Fifty-odd steps is well
    // past what the rest of the model can tell apart, and the closed form for this is
    // a page of elliptic integrals.
    const SAMPLES: usize = 64;
    let tilt = obliquity.to_radians();
    let mut total = 0.0;
    for i in 0..SAMPLES {
        // Declination through the year. Circular orbit: eccentricity's effect on the
        // annual mean is second order, and where it matters — the precession cycle — it
        // is a seasonal effect rather than an annual-mean one.
        let season = 2.0 * PI * (i as f64 + 0.5) / SAMPLES as f64;
        let declination = (tilt.sin() * season.sin()).asin();
        total += daily_mean(latitude, declination, solar_constant);
    }
    total / SAMPLES as f64
}

/// Insolation averaged over one rotation, at a given solar declination.
///
/// The textbook expression: integrate the cosine of the solar zenith angle over the
/// part of the day the sun is up, and divide by the whole day.
pub fn daily_mean(latitude: f64, declination: f64, solar_constant: f64) -> f64 {
    let (sin_lat, cos_lat) = latitude.sin_cos();
    let (sin_dec, cos_dec) = declination.sin_cos();

    // The hour angle at which the sun sets. Outside the tropics of the moment this
    // saturates, which is polar day and polar night.
    let cos_hour = -(sin_lat * sin_dec) / (cos_lat * cos_dec).max(1e-12);
    let hour = if cos_hour <= -1.0 {
        PI // sun never sets
    } else if cos_hour >= 1.0 {
        0.0 // sun never rises
    } else {
        cos_hour.acos()
    };

    solar_constant / PI * (hour * sin_lat * sin_dec + cos_lat * cos_dec * hour.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EARTH: f64 = SOLAR_CONSTANT;

    #[test]
    fn the_global_mean_is_a_quarter_of_the_solar_constant() {
        // Geometry, not physics: a sphere intercepts a disc's worth of light and spreads
        // it over four times the area. Anything else means the integration is wrong.
        let mut total = 0.0;
        let mut weight = 0.0;
        const BANDS: usize = 2000;
        for i in 0..BANDS {
            // Equal-area bands, so the weighting is uniform in the sine of latitude.
            let sin_lat = -1.0 + 2.0 * (i as f64 + 0.5) / BANDS as f64;
            let lat = sin_lat.asin();
            total += annual_mean(lat, EARTH_OBLIQUITY, EARTH);
            weight += 1.0;
        }
        let mean = total / weight;
        assert!(
            (mean - EARTH / 4.0).abs() < 1.0,
            "global mean insolation was {mean:.1}, expected {:.1}",
            EARTH / 4.0
        );
    }

    #[test]
    fn the_equator_gets_more_than_the_poles() {
        let equator = annual_mean(0.0, EARTH_OBLIQUITY, EARTH);
        let pole = annual_mean(PI / 2.0, EARTH_OBLIQUITY, EARTH);
        assert!(equator > pole * 2.0, "{equator:.0} against {pole:.0}");
        // And the real numbers, which are well known: about 417 at the equator and
        // 174 at the pole.
        assert!((equator - 417.0).abs() < 12.0, "equator {equator:.0}");
        assert!((pole - 174.0).abs() < 12.0, "pole {pole:.0}");
    }

    #[test]
    fn without_a_tilt_the_poles_are_nearly_dark() {
        let pole = annual_mean(PI / 2.0, 0.0, EARTH);
        assert!(pole < 1.0, "an untilted pole received {pole:.1}");
        // And the equator gets the most it ever can.
        let equator = annual_mean(0.0, 0.0, EARTH);
        assert!((equator - EARTH / PI).abs() < 1.0, "equator {equator:.1}");
    }

    #[test]
    fn past_fifty_four_degrees_of_tilt_the_poles_outshine_the_equator() {
        // A real and slightly startling result: a planet lying far enough on its side
        // has its coldest region at the equator. Uranus is past this. It is the sort of
        // thing a fitted `S₂` coefficient cannot produce at all.
        let flipped =
            |tilt: f64| annual_mean(PI / 2.0, tilt, EARTH) > annual_mean(0.0, tilt, EARTH);
        assert!(!flipped(40.0), "at 40° the poles should still be colder");
        assert!(flipped(70.0), "at 70° the poles should be warmer");
    }

    #[test]
    fn more_tilt_moves_sunlight_polewards() {
        let pole = |tilt| annual_mean(PI / 2.0, tilt, EARTH);
        assert!(pole(10.0) < pole(23.44));
        assert!(pole(23.44) < pole(35.0));
        // Which is exactly the lever behind glacial cycles: a couple of degrees of
        // obliquity is tens of watts at the latitudes where ice sheets live.
        let sixty = |tilt| annual_mean(60f64.to_radians(), tilt, EARTH);
        assert!(sixty(24.5) - sixty(22.0) > 3.0);
    }

    #[test]
    fn polar_night_receives_nothing() {
        // Mid-winter at the pole: the sun is below the horizon all day.
        let winter = daily_mean(PI / 2.0, -EARTH_OBLIQUITY.to_radians(), EARTH);
        assert_eq!(winter, 0.0);
        // And mid-summer there is bright — brighter than the equator ever is in a day,
        // because the sun never sets.
        let summer = daily_mean(PI / 2.0, EARTH_OBLIQUITY.to_radians(), EARTH);
        assert!(summer > 500.0, "polar midsummer was {summer:.0}");
    }

    #[test]
    fn the_tilt_wanders_but_averages_out() {
        // The Milankovitch pacemaker. Over a full cycle the mean is the mean; within one,
        // it is worth a couple of degrees, which is worth tens of watts where the ice is.
        let mean = EARTH_OBLIQUITY;
        let samples: Vec<f64> = (0..200)
            .map(|i| obliquity_at(i as f64 * TILT_PERIOD_MYR / 1000.0 / 200.0 * 4.0, mean))
            .collect();
        let low = samples.iter().copied().fold(f64::MAX, f64::min);
        let high = samples.iter().copied().fold(f64::MIN, f64::max);
        assert!(
            (high - low) > 2.0 && (high - low) < 3.0,
            "the tilt swung {:.2}°",
            high - low
        );
        let average = samples.iter().sum::<f64>() / samples.len() as f64;
        assert!((average - mean).abs() < 0.2, "it averaged {average:.2}°");

        // And a couple of degrees is worth a few watts a square metre at sixty degrees of
        // latitude, which is where the ice sheets live. Small, and enough: the whole
        // Milankovitch argument is that a few watts, applied for twenty thousand years to
        // the summer that has to melt last winter's snow, decides whether the ice grows.
        let at = |tilt| annual_mean(60f64.to_radians(), tilt, EARTH);
        let worth = at(high) - at(low);
        assert!(
            (2.0..8.0).contains(&worth),
            "the tilt swing was worth {worth:.1} W/m² at 60°"
        );
    }

    #[test]
    fn the_young_sun_was_faint() {
        assert!(
            (brightness_at(4.57) - 1.0).abs() < 1e-9,
            "today is the unit"
        );
        let early = brightness_at(0.5);
        assert!(
            (0.70..0.78).contains(&early),
            "half a gigayear in, the sun was {early:.3} of today"
        );
        // And it goes on brightening.
        assert!(brightness_at(5.5) > 1.0);
    }
}
