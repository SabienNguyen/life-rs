//! Simulated time, and the ladder of scales that makes deep time reachable.
//!
//! You cannot get to a million years by ticking faster. At the 15-minute step an agent
//! needs, a megayear is 3.5e10 ticks, and no amount of optimisation closes a gap that
//! size. So scales run *different integrators over different state*, and the cost works
//! out from both ends: coarse scales are cheap because they are coarse, and fine scales
//! are cheap because they only run in the window being watched.
//!
//! Time is counted in whole simulated seconds. Integer arithmetic means no accumulated
//! floating-point drift across the 1e13 steps a long run takes.

use std::fmt;
use std::ops::{Add, AddAssign, Mul, Sub};

/// A span of simulated time, in seconds.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Duration(u64);

impl Duration {
    pub const ZERO: Duration = Duration(0);

    pub const fn from_secs(secs: u64) -> Self {
        Duration(secs)
    }
    pub const fn from_minutes(minutes: u64) -> Self {
        Duration(minutes * 60)
    }
    pub const fn from_hours(hours: u64) -> Self {
        Duration(hours * 3_600)
    }
    /// Nominal 24-hour day. A planet's actual day length lives in its calendar.
    pub const fn from_days(days: u64) -> Self {
        Duration(days * 86_400)
    }
    /// Julian year (365.25 d) — the reference for every scale above `Day`.
    pub const fn from_years(years: u64) -> Self {
        Duration(years * SECONDS_PER_YEAR)
    }
    pub const fn from_kyr(kyr: u64) -> Self {
        Duration::from_years(kyr * 1_000)
    }
    pub const fn from_myr(myr: u64) -> Self {
        Duration::from_years(myr * 1_000_000)
    }

    pub const fn as_secs(self) -> u64 {
        self.0
    }
    pub fn as_days(self) -> f64 {
        self.0 as f64 / 86_400.0
    }
    pub fn as_years(self) -> f64 {
        self.0 as f64 / SECONDS_PER_YEAR as f64
    }
    pub fn as_myr(self) -> f64 {
        self.as_years() / 1.0e6
    }
}

pub const SECONDS_PER_YEAR: u64 = 31_557_600; // 365.25 days

impl Add for Duration {
    type Output = Duration;
    fn add(self, rhs: Duration) -> Duration {
        Duration(self.0 + rhs.0)
    }
}
impl Sub for Duration {
    type Output = Duration;
    fn sub(self, rhs: Duration) -> Duration {
        Duration(self.0 - rhs.0)
    }
}
impl Mul<u64> for Duration {
    type Output = Duration;
    fn mul(self, rhs: u64) -> Duration {
        Duration(self.0 * rhs)
    }
}

impl fmt::Debug for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.0;
        if secs >= SECONDS_PER_YEAR * 1_000_000 {
            write!(f, "{:.2} Myr", self.as_myr())
        } else if secs >= SECONDS_PER_YEAR * 1_000 {
            write!(f, "{:.2} kyr", self.as_years() / 1000.0)
        } else if secs >= SECONDS_PER_YEAR {
            write!(f, "{:.1} yr", self.as_years())
        } else if secs >= 86_400 {
            write!(f, "{:.1} d", self.as_days())
        } else if secs >= 3_600 {
            write!(f, "{:.1} h", secs as f64 / 3600.0)
        } else {
            write!(f, "{secs} s")
        }
    }
}

/// A moment, in seconds since the world formed.
///
/// u64 seconds reaches ~584 billion years, so the ceiling is not a design concern.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Time(u64);

impl Time {
    pub const ORIGIN: Time = Time(0);

    pub const fn from_secs(secs: u64) -> Self {
        Time(secs)
    }
    pub const fn as_secs(self) -> u64 {
        self.0
    }
    pub fn since(self, earlier: Time) -> Duration {
        Duration(self.0.saturating_sub(earlier.0))
    }
    /// Seconds elapsed within the current period of `length`.
    pub fn phase_within(self, length: Duration) -> u64 {
        debug_assert!(length.0 > 0, "zero-length period");
        self.0 % length.0
    }
    /// How many whole periods of `length` have elapsed.
    pub fn periods_of(self, length: Duration) -> u64 {
        debug_assert!(length.0 > 0, "zero-length period");
        self.0 / length.0
    }
    /// The next instant strictly after `self` that lands on a multiple of `length`.
    pub fn next_boundary(self, length: Duration) -> Time {
        Time((self.periods_of(length) + 1) * length.0)
    }
}

impl Add<Duration> for Time {
    type Output = Time;
    fn add(self, rhs: Duration) -> Time {
        Time(self.0 + rhs.0)
    }
}
impl AddAssign<Duration> for Time {
    fn add_assign(&mut self, rhs: Duration) {
        self.0 += rhs.0;
    }
}
impl Sub<Duration> for Time {
    type Output = Time;
    fn sub(self, rhs: Duration) -> Time {
        Time(self.0.saturating_sub(rhs.0))
    }
}
impl Sub<Time> for Time {
    type Output = Duration;
    fn sub(self, rhs: Time) -> Duration {
        self.since(rhs)
    }
}

impl fmt::Debug for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t+{}", Duration(self.0))
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Duration(self.0))
    }
}

/// The rungs of the ladder. Each runs its own integrator over its own state.
///
/// Crossing a rung is a *projection*: downward samples individuals from distributions,
/// upward aggregates individuals back into them. Both directions must preserve
/// aggregates — statistics are the contract, individuals are the implementation.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum TimeScale {
    /// Agent decisions, needs, interactions.
    Moment,
    /// Households, foraging, weather realisation.
    Day,
    /// Vegetation growth, migration, harvest, disturbance.
    Season,
    /// Demography, economy, settlement, succession.
    Generation,
    /// Community turnover, range shifts, soil development.
    Ecological,
    /// Selection, drift, mutation, gene flow, speciation.
    Evolutionary,
    /// Milankovitch forcing, glacial cycles, sea level.
    Orbital,
    /// Plate motion, orogeny, erosion, outgassing.
    Geological,
}

impl TimeScale {
    pub const ALL: [TimeScale; 8] = [
        TimeScale::Moment,
        TimeScale::Day,
        TimeScale::Season,
        TimeScale::Generation,
        TimeScale::Ecological,
        TimeScale::Evolutionary,
        TimeScale::Orbital,
        TimeScale::Geological,
    ];

    /// The nominal step. Integrators may take shorter steps through a transition —
    /// see [`TimeScale::adaptive_step`].
    pub const fn step(self) -> Duration {
        match self {
            TimeScale::Moment => Duration::from_minutes(15),
            TimeScale::Day => Duration::from_days(1),
            TimeScale::Season => Duration::from_secs(SECONDS_PER_YEAR / 4),
            TimeScale::Generation => Duration::from_years(1),
            TimeScale::Ecological => Duration::from_years(100),
            TimeScale::Evolutionary => Duration::from_kyr(1),
            TimeScale::Orbital => Duration::from_kyr(10),
            TimeScale::Geological => Duration::from_myr(1),
        }
    }

    /// Step size scaled by how fast the system is currently changing.
    ///
    /// `activity` runs 0 (quiescent) to 1 (a deglaciation, an anoxic event, a plate
    /// reorganisation). Cheap where nothing happens, careful where it does — which is
    /// also where everything interesting happens.
    pub fn adaptive_step(self, activity: f64) -> Duration {
        const MAX_REFINEMENT: f64 = 10.0;
        let activity = activity.clamp(0.0, 1.0);
        let divisor = 1.0 + activity * (MAX_REFINEMENT - 1.0);
        let secs = (self.step().as_secs() as f64 / divisor) as u64;
        Duration::from_secs(secs.max(1))
    }

    pub const fn label(self) -> &'static str {
        match self {
            TimeScale::Moment => "moment",
            TimeScale::Day => "day",
            TimeScale::Season => "season",
            TimeScale::Generation => "generation",
            TimeScale::Ecological => "ecological",
            TimeScale::Evolutionary => "evolutionary",
            TimeScale::Orbital => "orbital",
            TimeScale::Geological => "geological",
        }
    }

    /// The next rung up, if there is one.
    pub fn coarser(self) -> Option<TimeScale> {
        let i = TimeScale::ALL.iter().position(|s| *s == self)?;
        TimeScale::ALL.get(i + 1).copied()
    }

    pub fn finer(self) -> Option<TimeScale> {
        let i = TimeScale::ALL.iter().position(|s| *s == self)?;
        i.checked_sub(1)
            .and_then(|j| TimeScale::ALL.get(j))
            .copied()
    }

    /// The coarsest rung whose step still resolves `span`. Picking a scale for a task
    /// is otherwise a guess.
    pub fn resolving(span: Duration) -> TimeScale {
        TimeScale::ALL
            .iter()
            .rev()
            .find(|scale| scale.step() <= span)
            .copied()
            .unwrap_or(TimeScale::Moment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_is_strictly_increasing() {
        for pair in TimeScale::ALL.windows(2) {
            assert!(
                pair[0].step() < pair[1].step(),
                "{:?} should be finer than {:?}",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn the_ladder_spans_minutes_to_megayears() {
        assert_eq!(TimeScale::Moment.step(), Duration::from_minutes(15));
        assert_eq!(TimeScale::Geological.step(), Duration::from_myr(1));

        // The gap that rules out ticking uniformly: ~3.5e10 moments per megayear.
        let ratio = TimeScale::Geological.step().as_secs() / TimeScale::Moment.step().as_secs();
        assert!(ratio > 30_000_000_000, "ratio was {ratio}");
    }

    #[test]
    fn a_megayear_is_a_thousand_evolutionary_steps() {
        let steps =
            TimeScale::Geological.step().as_secs() / TimeScale::Evolutionary.step().as_secs();
        assert_eq!(steps, 1_000, "this is why deep time is affordable");
    }

    #[test]
    fn rungs_connect() {
        assert_eq!(TimeScale::Moment.finer(), None);
        assert_eq!(TimeScale::Geological.coarser(), None);
        assert_eq!(TimeScale::Day.coarser(), Some(TimeScale::Season));
        assert_eq!(TimeScale::Day.finer(), Some(TimeScale::Moment));
    }

    #[test]
    fn adaptive_stepping_refines_under_stress() {
        let calm = TimeScale::Orbital.adaptive_step(0.0);
        let crisis = TimeScale::Orbital.adaptive_step(1.0);
        assert_eq!(calm, TimeScale::Orbital.step());
        assert_eq!(crisis.as_secs(), TimeScale::Orbital.step().as_secs() / 10);
        assert!(crisis < calm);
    }

    #[test]
    fn choosing_a_scale_for_a_span() {
        assert_eq!(
            TimeScale::resolving(Duration::from_hours(2)),
            TimeScale::Moment
        );
        assert_eq!(
            TimeScale::resolving(Duration::from_years(3)),
            TimeScale::Generation
        );
        assert_eq!(
            TimeScale::resolving(Duration::from_myr(5)),
            TimeScale::Geological
        );
        // Shorter than the finest rung still has to answer something.
        assert_eq!(
            TimeScale::resolving(Duration::from_secs(1)),
            TimeScale::Moment
        );
    }

    #[test]
    fn time_arithmetic_round_trips() {
        let start = Time::ORIGIN + Duration::from_days(3);
        let later = start + Duration::from_hours(6);
        assert_eq!(later - start, Duration::from_hours(6));
        assert_eq!(later.since(start), Duration::from_hours(6));
        // Going back before the origin saturates rather than wrapping.
        assert_eq!(Time::ORIGIN - Duration::from_days(1), Time::ORIGIN);
    }

    #[test]
    fn period_boundaries() {
        let day = Duration::from_days(1);
        let t = Time::from_secs(86_400 * 2 + 3_600 * 7);
        assert_eq!(t.periods_of(day), 2);
        assert_eq!(t.phase_within(day), 3_600 * 7);
        assert_eq!(t.next_boundary(day), Time::from_secs(86_400 * 3));

        // Exactly on a boundary advances to the next one, never stalls.
        let exact = Time::from_secs(86_400 * 2);
        assert_eq!(exact.next_boundary(day), Time::from_secs(86_400 * 3));
    }

    #[test]
    fn deep_time_does_not_overflow() {
        let four_billion_years = Duration::from_myr(4_000);
        let t = Time::ORIGIN + four_billion_years;
        assert!((t.since(Time::ORIGIN).as_myr() - 4000.0).abs() < 1.0);
    }

    #[test]
    fn durations_read_at_the_right_magnitude() {
        assert_eq!(Duration::from_secs(30).to_string(), "30 s");
        assert_eq!(Duration::from_hours(5).to_string(), "5.0 h");
        assert_eq!(Duration::from_days(2).to_string(), "2.0 d");
        assert_eq!(Duration::from_myr(12).to_string(), "12.00 Myr");
    }
}
