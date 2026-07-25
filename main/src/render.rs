//! Turning recorded events into sentences.
//!
//! The simulation never phrases anything. Keeping the wording here means the chronicle
//! stays comparable between runs, and it is where a richer narrator will eventually go.

use person::Remark;
use planet::{Calendar, DayPhase, PlanetId};
use sim::{Happening, World};
use sim_core::{Record, Time};

pub fn line(world: &World, record: &Record<Happening>) -> String {
    // An event whose subject has since been removed still has to render — the handle
    // failing to resolve is normal, not exceptional. Fall back to the first planet's
    // calendar, then to no stamp at all.
    let calendar = context_planet(world, record.kind)
        .and_then(|id| world.planets.get(id))
        .or_else(|| world.planets.iter().next().map(|(_, p)| p))
        .map(|planet| planet.calendar);

    match calendar {
        Some(calendar) => format!(
            "{} {}",
            timestamp(&calendar, record.at),
            sentence(world, record.kind)
        ),
        None => sentence(world, record.kind),
    }
}

/// Whose clock this event should be stamped with.
fn context_planet(world: &World, happening: Happening) -> Option<PlanetId> {
    match happening {
        Happening::WorldBegins { planet } | Happening::PhaseBegins { planet, .. } => Some(planet),
        Happening::PersonArrives { person } | Happening::PersonRemarks { person, .. } => {
            world.people.get(person).map(|p| p.home)
        }
    }
}

fn sentence(world: &World, happening: Happening) -> String {
    match happening {
        Happening::WorldBegins { planet } => {
            let name = planet_name(world, planet);
            format!("Hello, I am planet {name}")
        }

        Happening::PhaseBegins { planet, phase } => {
            let name = planet_name(world, planet);
            match phase {
                DayPhase::Morning => format!("It is now morning on planet {name}"),
                DayPhase::Afternoon => format!("It is now the afternoon on planet {name}"),
                DayPhase::Evening => format!("It is now evening on planet {name}"),
                DayPhase::Night => format!("It is now nighttime on planet {name}"),
            }
        }

        Happening::PersonArrives { person } => match world.people.get(person) {
            Some(p) => format!("Hi! My name is {} and I am from {}.", p.name, p.country),
            None => "Someone arrives.".to_string(),
        },

        Happening::PersonRemarks { person, remark } => {
            let who = world
                .people
                .get(person)
                .map(|p| p.name.as_str())
                .unwrap_or("Someone");
            format!("{who} says \"{}\"", words(remark))
        }
    }
}

fn words(remark: Remark) -> &'static str {
    match remark {
        Remark::Bored => "Good morning! I am bored now...",
        Remark::Lunch => "It is the afternoon now, I will eat lunch!",
        Remark::Dinner => "It is the evening now, I will eat dinner!",
        Remark::GoodNight => "It is nighttime now... Good night.",
    }
}

fn planet_name(world: &World, id: PlanetId) -> &str {
    world
        .planets
        .get(id)
        .map(|p| p.name.as_str())
        .unwrap_or("?")
}

/// A compact local-time stamp. The year only appears once there is one.
fn timestamp(calendar: &Calendar, at: Time) -> String {
    let date = calendar.date_at(at);
    let hours = date.second_of_day / 3_600;
    let minutes = (date.second_of_day % 3_600) / 60;
    if date.year == 0 {
        format!("[day {:<3} {hours:02}:{minutes:02}]", date.day_of_year + 1)
    } else {
        format!(
            "[yr {} day {:<3} {hours:02}:{minutes:02}]",
            date.year,
            date.day_of_year + 1
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Duration, Salience, WorldSeed};

    fn world_with_transcript() -> (World, Vec<String>) {
        let mut world = World::genesis(WorldSeed::from_u128(1), 1);
        world.run_for(Duration::from_hours(30));
        let lines: Vec<String> = world
            .chronicle
            .at_least(Salience::Routine)
            .map(|r| line(&world, r))
            .collect();
        (world, lines)
    }

    #[test]
    fn the_original_wording_survives_the_port() {
        let (_world, lines) = world_with_transcript();
        let spoken: Vec<&str> = lines
            .iter()
            .map(|l| l.split_once("] ").map(|(_, rest)| rest).unwrap_or(l))
            .collect();

        assert_eq!(spoken[1], "Hello, I am planet Earth");
        assert_eq!(spoken[3], "It is now morning on planet Earth");
        assert_eq!(spoken[5], "It is now the afternoon on planet Earth");
        assert_eq!(spoken[7], "It is now evening on planet Earth");
        assert_eq!(spoken[9], "It is now nighttime on planet Earth");

        assert!(spoken[0].starts_with("Hi! My name is "));
        assert!(spoken[0].ends_with('.'));
        assert!(spoken[2].contains("Good morning! I am bored now..."));
        assert!(spoken[4].contains("It is the afternoon now, I will eat lunch!"));
        assert!(spoken[6].contains("It is the evening now, I will eat dinner!"));
        assert!(spoken[8].contains("It is nighttime now... Good night."));
    }

    #[test]
    fn events_are_stamped_with_local_time() {
        let (_world, lines) = world_with_transcript();
        assert!(
            lines[0].starts_with("[day 1   00:00]"),
            "got {:?}",
            lines[0]
        );
        assert!(
            lines[2].starts_with("[day 1   06:00]"),
            "got {:?}",
            lines[2]
        );
        assert!(
            lines[10].starts_with("[day 2   06:00]"),
            "got {:?}",
            lines[10]
        );
    }

    #[test]
    fn the_year_appears_once_there_is_one() {
        let calendar = Calendar::EARTH;
        let later = Time::ORIGIN + Duration::from_days(400);
        assert!(timestamp(&calendar, later).starts_with("[yr 1 day 36"));
    }

    #[test]
    fn rendering_survives_a_vanished_subject() {
        let mut world = World::genesis(WorldSeed::from_u128(5), 1);
        world.run_for(Duration::from_hours(13));
        let id = world.people.ids().next().unwrap();
        world.people.remove(id);

        // The events they left behind must still render rather than panicking.
        for record in world.chronicle.iter() {
            let text = line(&world, record);
            assert!(!text.is_empty());
        }
    }
}
