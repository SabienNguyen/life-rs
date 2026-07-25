//! Planets, and the calendars that turn simulated time into local time.
//!
//! The day/night state machine this crate used to carry is gone. Time of day is now
//! *derived* from the clock and the planet's rotation period, which is the rule the
//! design leans on everywhere: store the quantity, read off the label. A stored phase
//! can drift out of step with the clock; a derived one cannot, and it answers "what
//! time is it there in ten thousand years" for free.

use sim_core::{Duration, Id, Time};

pub type PlanetId = Id<Planet>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Classification {
    Terrestial,
    Jovian,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    ExtraLarge,
    Large,
    Normal,
    Small,
    ExtraSmall,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moon {
    pub name: String,
}

impl Moon {
    pub fn new(name: impl Into<String>) -> Moon {
        Moon { name: name.into() }
    }
}

/// Quarters of the local day.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DayPhase {
    Night,
    Morning,
    Afternoon,
    Evening,
}

impl DayPhase {
    pub const fn label(self) -> &'static str {
        match self {
            DayPhase::Night => "night",
            DayPhase::Morning => "morning",
            DayPhase::Afternoon => "afternoon",
            DayPhase::Evening => "evening",
        }
    }
}

/// A local date: which year, which day of it, how far into the day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Date {
    pub year: u64,
    pub day_of_year: u32,
    pub second_of_day: u64,
}

/// How a planet's rotation and orbit divide up simulated time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calendar {
    pub day_length: Duration,
    pub days_per_year: u32,
}

impl Calendar {
    pub const EARTH: Calendar = Calendar {
        day_length: Duration::from_hours(24),
        days_per_year: 365,
    };

    pub const fn new(day_length: Duration, days_per_year: u32) -> Calendar {
        Calendar {
            day_length,
            days_per_year,
        }
    }

    fn quarter(&self) -> Duration {
        Duration::from_secs((self.day_length.as_secs() / 4).max(1))
    }

    /// Time of day, read off the clock. The origin of time is local midnight, so the
    /// first phase a world ever sees is morning.
    pub fn phase_at(&self, t: Time) -> DayPhase {
        match t.phase_within(self.day_length) / self.quarter().as_secs() {
            0 => DayPhase::Night,
            1 => DayPhase::Morning,
            2 => DayPhase::Afternoon,
            // A day length not divisible by four leaves a remainder; evening absorbs it.
            _ => DayPhase::Evening,
        }
    }

    /// The next instant at which the phase changes.
    pub fn next_phase_change(&self, t: Time) -> Time {
        t.next_boundary(self.quarter())
    }

    pub fn date_at(&self, t: Time) -> Date {
        let day_index = t.periods_of(self.day_length);
        let days_per_year = u64::from(self.days_per_year.max(1));
        Date {
            year: day_index / days_per_year,
            day_of_year: (day_index % days_per_year) as u32,
            second_of_day: t.phase_within(self.day_length),
        }
    }

    pub fn year_length(&self) -> Duration {
        self.day_length * u64::from(self.days_per_year)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planet {
    pub name: String,
    pub size: Size,
    pub livable: bool,
    pub classification: Classification,
    pub calendar: Calendar,
    pub moons: Vec<Moon>,
}

impl Planet {
    pub fn new(
        name: impl Into<String>,
        size: Size,
        livable: bool,
        classification: Classification,
        calendar: Calendar,
        moons: Vec<Moon>,
    ) -> Planet {
        Planet {
            name: name.into(),
            size,
            livable,
            classification,
            calendar,
            moons,
        }
    }

    /// An Earth-like world — the default, and the fixture most tests want.
    pub fn earth() -> Planet {
        Planet::new(
            "Earth",
            Size::Normal,
            true,
            Classification::Terrestial,
            Calendar::EARTH,
            vec![Moon::new("Moon")],
        )
    }

    pub fn phase_at(&self, t: Time) -> DayPhase {
        self.calendar.phase_at(t)
    }

    pub fn next_phase_change(&self, t: Time) -> Time {
        self.calendar.next_phase_change(t)
    }

    pub fn date_at(&self, t: Time) -> Date {
        self.calendar.date_at(t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_hours(h: u64) -> Time {
        Time::ORIGIN + Duration::from_hours(h)
    }

    #[test]
    fn the_day_divides_into_quarters() {
        let c = Calendar::EARTH;
        assert_eq!(c.phase_at(at_hours(0)), DayPhase::Night);
        assert_eq!(c.phase_at(at_hours(5)), DayPhase::Night);
        assert_eq!(c.phase_at(at_hours(6)), DayPhase::Morning);
        assert_eq!(c.phase_at(at_hours(11)), DayPhase::Morning);
        assert_eq!(c.phase_at(at_hours(12)), DayPhase::Afternoon);
        assert_eq!(c.phase_at(at_hours(18)), DayPhase::Evening);
        assert_eq!(c.phase_at(at_hours(23)), DayPhase::Evening);
    }

    #[test]
    fn phases_repeat_the_next_day_and_the_next_millennium() {
        let c = Calendar::EARTH;
        assert_eq!(c.phase_at(at_hours(30)), DayPhase::Morning);
        // Derived, not accumulated: no drift however far out we look.
        let far = Time::ORIGIN + Duration::from_years(1_000) + Duration::from_hours(13);
        assert_eq!(c.phase_at(far), DayPhase::Afternoon);
    }

    #[test]
    fn phase_changes_land_on_quarter_boundaries() {
        let c = Calendar::EARTH;
        assert_eq!(c.next_phase_change(at_hours(0)), at_hours(6));
        assert_eq!(c.next_phase_change(at_hours(3)), at_hours(6));
        // On a boundary, advance to the next one rather than stalling there.
        assert_eq!(c.next_phase_change(at_hours(6)), at_hours(12));
        assert_eq!(c.next_phase_change(at_hours(23)), at_hours(24));
    }

    #[test]
    fn walking_the_boundaries_visits_every_phase_in_order() {
        let c = Calendar::EARTH;
        let mut t = Time::ORIGIN;
        let mut seen = Vec::new();
        for _ in 0..8 {
            t = c.next_phase_change(t);
            seen.push(c.phase_at(t));
        }
        use DayPhase::*;
        assert_eq!(
            seen,
            vec![
                Morning, Afternoon, Evening, Night, Morning, Afternoon, Evening, Night
            ]
        );
    }

    #[test]
    fn dates_count_days_and_years() {
        let c = Calendar::EARTH;
        let d = c.date_at(at_hours(24 * 400 + 7));
        assert_eq!(d.year, 1);
        assert_eq!(d.day_of_year, 35);
        assert_eq!(d.second_of_day, 7 * 3_600);
    }

    #[test]
    fn other_worlds_keep_their_own_time() {
        // A short-day world: quarters are three hours, not six.
        let brisk = Calendar::new(Duration::from_hours(12), 400);
        assert_eq!(brisk.phase_at(at_hours(2)), DayPhase::Night);
        assert_eq!(brisk.phase_at(at_hours(4)), DayPhase::Morning);
        assert_eq!(brisk.next_phase_change(at_hours(0)), at_hours(3));
        assert_eq!(brisk.year_length(), Duration::from_hours(12 * 400));
    }

    #[test]
    fn earth_is_earth_like() {
        let earth = Planet::earth();
        assert_eq!(earth.name, "Earth");
        assert!(earth.livable);
        assert_eq!(earth.moons.len(), 1);
        assert_eq!(earth.phase_at(at_hours(9)), DayPhase::Morning);
    }
}
