//! Turning recorded events into sentences.
//!
//! The simulation never phrases anything. Keeping the wording here means the chronicle
//! stays comparable between runs, and it is where a richer narrator will eventually go.

use person::{Cause, Deed, Person};
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
        _ => happening
            .subject()
            .and_then(|id| world.people.get(id))
            .map(|p| p.home),
    }
}

fn sentence(world: &World, happening: Happening) -> String {
    match happening {
        Happening::WorldBegins { planet } => {
            format!("Hello, I am planet {}", planet_name(world, planet))
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
            Some(p) => format!(
                "Hi! My name is {} and I am from {}. I am {}, and {}.",
                p.name,
                p.country,
                describe_age(world, p),
                p.personality.outlook().label(),
            ),
            None => "Someone arrives.".to_string(),
        },

        Happening::PersonDoes { person, deed } => {
            format!("{} is {}", who(world, person), deed.label())
        }

        Happening::PersonDies { person, cause } => match cause {
            Cause::OldAge => format!("{} dies of old age", who(world, person)),
            other => format!("{} dies of {}", who(world, person), other.label()),
        },

        Happening::PersonMoves { person, to } => format!(
            "{} moves to {}",
            who(world, person),
            world
                .places
                .get(to)
                .map(|p| p.name.as_str())
                .unwrap_or("elsewhere")
        ),

        Happening::PersonMentored { person } => {
            format!("{} finds someone willing to open doors", who(world, person))
        }

        Happening::PlaceChanges { place, into } => format!(
            "{} has become {}",
            world
                .places
                .get(place)
                .map(|p| p.name.as_str())
                .unwrap_or("somewhere"),
            into.label()
        ),

        Happening::PersonPairs { person, with } => format!(
            "{} and {} set up house together",
            who(world, person),
            who(world, with)
        ),

        Happening::PersonBorn {
            child,
            mother,
            father,
        } => {
            let heritage = world
                .people
                .get(child)
                .map(|c| {
                    let openness = c.origins.openness;
                    format!(
                        " (openness {:+.2}: {:+.2} inherited, {:+.2} from home)",
                        openness.total(),
                        openness.genetic,
                        openness.shared
                    )
                })
                .unwrap_or_default();
            format!(
                "{} is born to {} and {}{heritage}",
                who(world, child),
                who(world, mother),
                who(world, father)
            )
        }
    }
}

fn who(world: &World, id: person::PersonId) -> &str {
    world
        .people
        .get(id)
        .map(|p| p.name.as_str())
        .unwrap_or("Someone")
}

fn describe_age(world: &World, p: &Person) -> String {
    format!("{} years old", p.age(world.now()).years().floor() as u64)
}

fn planet_name(world: &World, id: PlanetId) -> &str {
    world
        .planets
        .get(id)
        .map(|p| p.name.as_str())
        .unwrap_or("?")
}

/// A one-line summary of a person as they stand — the seed of a dossier.
pub fn portrait(world: &World, id: person::PersonId) -> String {
    let Some(p) = world.people.get(id) else {
        return "(nobody)".to_string();
    };
    let now = world.now();

    let state = match p.death() {
        Some((_, cause)) => format!("died of {}", cause.label()),
        None => match p.intent() {
            Some(intent) => format!("{}, {} to go", intent.deed.label(), intent.remaining(now)),
            None => "idle".to_string(),
        },
    };

    let (need, pressure) = p.needs().most_pressing();
    format!(
        "{} — {}, {} {}, {} — {} (most pressing: {} {:.0}%, health {:.0}%)",
        p.name,
        describe_age(world, p),
        p.stage(now).label(),
        p.country,
        p.personality.outlook().label(),
        state,
        need,
        pressure * 100.0,
        p.health().vitality * 100.0,
    )
}

/// Where a person's temperament came from, factor by factor.
///
/// The whole reason genes, household, and chance are carried separately rather than
/// summed: with them apart, "why is she like that" has an answer, and "what if she had
/// been raised elsewhere" is a substitution rather than another lifetime.
pub fn heritage(world: &World, id: person::PersonId) -> Vec<String> {
    let Some(p) = world.people.get(id) else {
        return Vec::new();
    };

    let mut lines: Vec<String> = p
        .origins
        .labelled()
        .iter()
        .map(|(name, e)| {
            format!(
                "  {name:<18} {:+.2}   = genes {:+.2}  home {:+.2}  chance {:+.2}",
                e.total(),
                e.genetic,
                e.shared,
                e.unique
            )
        })
        .collect();

    // The counterfactual, free because the parts were never merged.
    let bleak = p.origins.if_raised(-1.5);
    let kind = p.origins.if_raised(1.5);
    lines.push(String::new());
    lines.push(format!(
        "  raised badly, conscientiousness would be {:+.2}; raised well, {:+.2} (is {:+.2})",
        bleak.conscientiousness, kind.conscientiousness, p.personality.conscientiousness
    ));
    lines
}

/// Immediate family, as far as it is recorded.
pub fn family(world: &World, id: person::PersonId) -> Vec<String> {
    let mut lines = Vec::new();

    match world.society.parents_of(id) {
        Some((mother, father)) => lines.push(format!(
            "  parents    {} and {}",
            who(world, mother),
            who(world, father)
        )),
        None => lines.push("  parents    unrecorded — of the founding generation".to_string()),
    }

    if let Some(partner) = world.society.partner_of(id) {
        lines.push(format!("  partner    {}", who(world, partner)));
    }

    let siblings = world.society.siblings_of(id);
    if !siblings.is_empty() {
        lines.push(format!(
            "  siblings   {}",
            siblings
                .iter()
                .map(|s| who(world, *s))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let children = world.society.children_of(id);
    if !children.is_empty() {
        lines.push(format!(
            "  children   {}",
            children
                .iter()
                .map(|c| who(world, *c))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let descendants = world.society.descendants_of(id).len();
    if descendants > children.len() {
        lines.push(format!("  lineage    {descendants} descendants in all"));
    }
    lines
}

/// Every neighbourhood, as it currently reads.
pub fn neighbourhoods(world: &World) -> Vec<String> {
    let mut lines = vec![format!(
        "  {:<13} {:<18} {:>6} {:>6} {:>6} {:>6} {:>6} {:>5}",
        "place", "reads as", "afflu", "safety", "bond", "bridge", "jobs", "hholds"
    )];
    for (id, place) in world.places.iter() {
        let households = world.society.households_in(id).count();
        lines.push(format!(
            "  {:<13} {:<18} {:>6.2} {:>6.2} {:>6.2} {:>6.2} {:>6.2} {:>5}",
            place.name,
            place.archetype().label(),
            place.env.affluence,
            place.env.safety,
            place.env.bonding_capital,
            place.env.bridging_capital,
            place.env.job_opportunity,
            households,
        ));
        // The ground under it, indented under the reading it produced — because the
        // point of the join is that the second line explains a good deal of the first.
        if let Some(terrain) = &place.terrain {
            lines.push(format!(
                "  {:<13} └ {}, soil {:.0}%, reach {:.0}%{}",
                "",
                terrain.describe(),
                terrain.fertility * 100.0,
                terrain.reach * 100.0,
                if terrain.hardship() > 0.4 {
                    format!(", a hard year ({:.0}%)", terrain.hardship() * 100.0)
                } else {
                    String::new()
                },
            ));
        }
    }
    lines
}

/// The planet a populated world stands on, in one paragraph.
pub fn ground(world: &World) -> Vec<String> {
    let Some(surface) = world.surface() else {
        return vec!["  (this world has no planet under it)".to_string()];
    };
    let planet = &surface.planet;
    let climate = &surface.climate;
    let star = surface.star();
    let orbit = surface.orbit();
    vec![
        format!(
            "  {} {} star of {:.2} solar masses, {:.2} Gyr old with {:.1} Gyr left",
            star.article(),
            star.colour(),
            star.mass_solar,
            star.age_gyr,
            star.remaining_gyr(),
        ),
        format!(
            "  {}",
            cosmos::habitability::describe(&star, &orbit),
        ),
        format!(
            "  a year is {:.2} Earth years; gravity {:.2} g",
            orbit.year_years(&star),
            orbit.gravity(),
        ),
        format!(
            "  {:.0}% land, {:.0}% of it in one mass, {} plates, highest point {:.0} m",
            planet.land_fraction() * 100.0,
            planet.largest_landmass_share() * 100.0,
            planet.active_plates(),
            planet
                .grid()
                .cells()
                .map(|c| planet.height_above_sea_m(c))
                .fold(f32::MIN, f32::max),
        ),
        format!(
            "  {:.1} °C on average, {:.0} ppm carbon dioxide, {:.0}% under ice, \
             {:.0} mm of rain a year",
            climate.mean_temperature_c(planet),
            climate.co2_ppm(),
            climate.ice_fraction(planet) * 100.0,
            climate.mean_rain_mm(planet),
        ),
        format!(
            "  {:.0}% forest, {:.0}% arid, {:.1} Gt of plant matter a year",
            surface.life.forest_share(planet) * 100.0,
            surface.life.desert_share(planet) * 100.0,
            surface.life.total_production_gt(planet),
        ),
    ]
}

/// The scoring table behind a person's current choice — an early `why()`.
pub fn reasoning(world: &World, id: person::PersonId) -> Vec<String> {
    let Some(p) = world.people.get(id) else {
        return Vec::new();
    };
    let Some(planet) = world.planets.get(p.home) else {
        return Vec::new();
    };

    let mut situation = person::Situation::plain(planet.phase_at(world.now()));
    situation.env.stress = p.needs().total_pressure();

    let scores = person::deeds::score_all(
        &person::Mind {
            personality: &p.personality,
            values: &p.values,
            needs: p.needs(),
            age_years: p.age(world.now()).years(),
        },
        &situation,
    );

    let mut ranked: Vec<(Deed, f32)> = Deed::ALL
        .into_iter()
        .map(|d| (d, scores[d as usize]))
        .collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));

    ranked
        .into_iter()
        .map(|(deed, score)| {
            if score <= 0.0 {
                format!("  {:<12} unavailable here", deed.label())
            } else {
                format!("  {:<12} {score:.3}", deed.label())
            }
        })
        .collect()
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

    fn a_world() -> World {
        let mut world = World::genesis(WorldSeed::from_u128(1), 1);
        world.run_for(Duration::from_days(2));
        world
    }

    #[test]
    fn the_planet_still_speaks_as_it_always_did() {
        let world = a_world();
        let spoken: Vec<String> = world
            .chronicle
            .iter()
            .map(|r| {
                let l = line(&world, r);
                l.split_once("] ")
                    .map(|(_, rest)| rest.to_string())
                    .unwrap_or(l)
            })
            .collect();

        assert!(spoken.contains(&"Hello, I am planet Earth".to_string()));
        assert!(spoken.contains(&"It is now morning on planet Earth".to_string()));
        assert!(spoken.contains(&"It is now the afternoon on planet Earth".to_string()));
        assert!(spoken.contains(&"It is now evening on planet Earth".to_string()));
        assert!(spoken.contains(&"It is now nighttime on planet Earth".to_string()));
    }

    #[test]
    fn a_person_introduces_themselves_with_who_they_are() {
        let world = a_world();
        let intro = world
            .chronicle
            .iter()
            .find(|r| matches!(r.kind, Happening::PersonArrives { .. }))
            .map(|r| line(&world, r))
            .expect("someone should arrive");

        assert!(intro.contains("Hi! My name is "));
        assert!(intro.contains("years old"));
    }

    #[test]
    fn deeds_read_as_sentences() {
        let world = a_world();
        let doings: Vec<String> = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonDoes { .. }))
            .map(|r| line(&world, r))
            .collect();

        assert!(!doings.is_empty());
        assert!(doings.iter().any(|d| d.contains(" is sleeping")));
        assert!(doings.iter().all(|d| d.starts_with('[')));
    }

    #[test]
    fn events_are_stamped_with_local_time() {
        let world = a_world();
        let first = line(&world, world.chronicle.iter().next().unwrap());
        // Worlds are founded with a century of history behind them, so the year shows,
        // and founding lands on midnight of a year boundary.
        assert!(first.starts_with("[yr 100 day 1   00:00]"), "got {first:?}");
    }

    #[test]
    fn a_portrait_says_who_and_how_someone_is() {
        let world = a_world();
        let id = world.people.ids().next().unwrap();
        let portrait = portrait(&world, id);

        assert!(portrait.contains("years old"));
        assert!(portrait.contains("most pressing:"));
        assert!(portrait.contains("health"));
    }

    #[test]
    fn the_reasoning_is_shown_and_ranked() {
        let world = a_world();
        let id = world.people.ids().next().unwrap();
        let lines = reasoning(&world, id);

        assert_eq!(lines.len(), Deed::COUNT);
        let scores: Vec<f32> = lines
            .iter()
            .filter_map(|l| l.split_whitespace().last()?.parse().ok())
            .collect();
        assert!(
            scores.windows(2).all(|w| w[0] >= w[1]),
            "should be ranked: {lines:?}"
        );
    }

    #[test]
    fn rendering_survives_a_vanished_subject() {
        let mut world = a_world();
        let id = world.people.ids().next().unwrap();
        world.people.remove(id);

        for record in world.chronicle.iter() {
            assert!(!line(&world, record).is_empty());
        }
        assert_eq!(portrait(&world, id), "(nobody)");
        assert!(reasoning(&world, id).is_empty());
    }

    #[test]
    fn a_death_is_narrated() {
        let mut world = World::genesis(WorldSeed::from_u128(3), 12);
        world.record_only(Salience::Pivotal);
        world.run_for(Duration::from_years(40));
        let obituary = world
            .chronicle
            .at_least(Salience::Pivotal)
            .filter(|r| matches!(r.kind, Happening::PersonDies { .. }))
            .map(|r| line(&world, r))
            .next()
            .expect("40 years should produce a death");
        assert!(obituary.contains(" dies of "), "got {obituary:?}");
    }
}
