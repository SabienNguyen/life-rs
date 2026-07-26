//! Writing a world out as JSON, so something other than a terminal can show it.
//!
//! Hand-rolled rather than pulling in a serialisation crate: the shape is small, it is
//! wanted in exactly one place, and a viewer is not a reason to put a dependency into
//! the simulation itself.

use observer::Balance;
use person::PersonId;
use sim::{Happening, World};
use sim_core::Salience;

use crate::render;

/// One year's reading of the world, for the charts.
pub struct YearSample {
    pub year: u64,
    pub living: usize,
    /// Affluence of each place, in arena order.
    pub affluence: Vec<f32>,
}

pub fn snapshot(world: &World, series: &[YearSample], balance: &Balance) -> String {
    let mut out = String::from("{\n");
    push(&mut out, "seed", &quoted(&world.seed.to_string()));
    push(&mut out, "year", &format!("{}", years_of(world)));
    push(&mut out, "living", &format!("{}", world.living()));
    push(&mut out, "everLived", &format!("{}", world.people.len()));
    push(
        &mut out,
        "factorNames",
        r#"["openness","conscientiousness","extraversion","agreeableness","neuroticism"]"#,
    );
    push(&mut out, "planet", &planet(world));
    push(&mut out, "places", &places(world));
    push(&mut out, "people", &people(world));
    push(&mut out, "events", &events(world));
    push(&mut out, "series", &samples(series));
    out.push_str(&format!("  \"balance\": {}\n", balance_json(balance)));
    out.push('}');
    out
}

fn years_of(world: &World) -> u64 {
    world
        .planets
        .iter()
        .next()
        .map(|(_, p)| p.date_at(world.now()).year)
        .unwrap_or(0)
}

/// The planet the world stands on, or `null` for a world that stands on nothing.
fn planet(world: &World) -> String {
    let Some(surface) = world.surface() else {
        return "null".to_string();
    };
    let planet = &surface.planet;
    let climate = &surface.climate;
    format!(
        "{{{}}}",
        [
            field("land", &num(planet.land_fraction())),
            field("biggestMass", &num(planet.largest_landmass_share())),
            field("plates", &format!("{}", planet.active_plates())),
            field(
                "peakM",
                &num(
                    planet
                        .grid()
                        .cells()
                        .map(|c| planet.height_above_sea_m(c))
                        .fold(f32::MIN, f32::max)
                )
            ),
            field("meanC", &num(climate.mean_temperature_c(planet))),
            field("co2Ppm", &num(climate.co2_ppm())),
            field("ice", &num(climate.ice_fraction(planet))),
            field("rainMm", &num(climate.mean_rain_mm(planet))),
            field("forest", &num(surface.life.forest_share(planet))),
            field("arid", &num(surface.life.desert_share(planet))),
            field("productionGt", &num(surface.life.total_production_gt(planet))),
            field("cells", &format!("{}", planet.grid().len())),
        ]
        .join(",")
    )
}

fn places(world: &World) -> String {
    let entries: Vec<String> = world
        .places
        .iter()
        .map(|(id, place)| {
            let households = world.society.households_in(id).count();
            let residents: usize = world
                .society
                .households_in(id)
                .flat_map(|(_, h)| h.members.iter())
                .filter(|m| world.people.get(**m).is_some_and(|p| p.is_alive()))
                .count();
            let e = &place.env;
            format!(
                "{{{}}}",
                [
                    field("name", &quoted(&place.name)),
                    field("archetype", &quoted(place.archetype().label())),
                    field("affluence", &num(e.affluence)),
                    field("safety", &num(e.safety)),
                    field("bonding", &num(e.bonding_capital)),
                    field("bridging", &num(e.bridging_capital)),
                    field("opportunity", &num(e.job_opportunity)),
                    field("schooling", &num(e.education_access)),
                    field("density", &num(e.density)),
                    field("churn", &num(e.churn)),
                    field("households", &format!("{households}")),
                    field("residents", &format!("{residents}")),
                    field("capacity", &format!("{}", place.capacity)),
                    // The ground, so the viewer can say why a quarter is what it is
                    // rather than only that it is.
                    field(
                        "ground",
                        &match &place.terrain {
                            Some(t) => quoted(&t.describe()),
                            None => "null".to_string(),
                        },
                    ),
                    field(
                        "fertility",
                        &place
                            .terrain
                            .as_ref()
                            .map_or("null".to_string(), |t| num(t.fertility)),
                    ),
                    field(
                        "reach",
                        &place
                            .terrain
                            .as_ref()
                            .map_or("null".to_string(), |t| num(t.reach)),
                    ),
                    field(
                        "hardship",
                        &place
                            .terrain
                            .as_ref()
                            .map_or("null".to_string(), |t| num(t.hardship())),
                    ),
                ]
                .join(",")
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

fn people(world: &World) -> String {
    let index = |id: PersonId| {
        world
            .people
            .ids()
            .position(|other| other == id)
            .map(|i| i.to_string())
            .unwrap_or_else(|| "null".to_string())
    };
    let place_index = |id: PersonId| {
        world
            .society
            .place_of(id)
            .and_then(|p| world.places.ids().position(|other| other == p))
            .map(|i| i.to_string())
            .unwrap_or_else(|| "null".to_string())
    };

    let entries: Vec<String> = world
        .people
        .iter()
        .map(|(id, p)| {
            let now = world.now();
            let age = match p.death() {
                Some((when, _)) => p.age(when).years(),
                None => p.age(now).years(),
            };
            let parents = match p.parents {
                Some((m, f)) => format!("[{},{}]", index(m), index(f)),
                None => "null".to_string(),
            };
            let children: Vec<String> = world
                .society
                .children_of(id)
                .iter()
                .map(|c| index(*c))
                .collect();
            // Flat arrays rather than objects: five numbers per factor, five factors
            // per person, and the field names are known to the viewer. Objects cost
            // roughly three times the bytes for exactly the same information.
            let factors: Vec<String> = p
                .origins
                .each()
                .iter()
                .map(|e| format!("[{},{},{}]", num(e.genetic), num(e.shared), num(e.unique)))
                .collect();

            format!(
                "{{{}}}",
                [
                    field("name", &quoted(&p.name)),
                    field("sex", &quoted(p.sex.label())),
                    field("age", &format!("{:.0}", age)),
                    field("alive", if p.is_alive() { "true" } else { "false" }),
                    field(
                        "died",
                        &match p.death() {
                            Some((_, cause)) => quoted(cause.label()),
                            None => "null".to_string(),
                        }
                    ),
                    field("outlook", &quoted(p.personality.outlook().label())),
                    field("country", &quoted(&p.country.to_string())),
                    field("standing", &num(p.peak_standing())),
                    field("mentored", if p.is_mentored() { "true" } else { "false" }),
                    field("upbringing", &num(p.absorbed_upbringing())),
                    field("place", &place_index(id)),
                    field("parents", &parents),
                    field("children", &format!("[{}]", children.join(","))),
                    field(
                        "partner",
                        &world
                            .society
                            .partner_of(id)
                            .map(index)
                            .unwrap_or_else(|| "null".to_string())
                    ),
                    field("factors", &format!("[{}]", factors.join(","))),
                ]
                .join(",")
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

fn events(world: &World) -> String {
    let calendar = world.planets.iter().next().map(|(_, p)| p.calendar);
    let entries: Vec<String> = world
        .chronicle
        .at_least(Salience::Pivotal)
        .map(|record| {
            let year = calendar.map(|c| c.date_at(record.at).year).unwrap_or(0);
            let kind = match record.kind {
                Happening::WorldBegins { .. } => "world",
                Happening::PersonBorn { .. } => "birth",
                Happening::PersonDies { .. } => "death",
                Happening::PersonPairs { .. } => "pairing",
                Happening::PersonMoves { .. } => "move",
                Happening::PersonMentored { .. } => "patron",
                Happening::PlaceChanges { .. } => "place",
                _ => "other",
            };
            format!(
                "{{{}}}",
                [
                    field("year", &format!("{year}")),
                    field("kind", &quoted(kind)),
                    field("text", &quoted(&plain(&render::line(world, record)))),
                ]
                .join(",")
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

fn samples(series: &[YearSample]) -> String {
    let entries: Vec<String> = series
        .iter()
        .map(|s| {
            let affluence: Vec<String> = s.affluence.iter().map(|a| num(*a)).collect();
            format!(
                "{{{}}}",
                [
                    field("year", &format!("{}", s.year)),
                    field("living", &format!("{}", s.living)),
                    field("affluence", &format!("[{}]", affluence.join(","))),
                ]
                .join(",")
            )
        })
        .collect();
    format!("[{}]", entries.join(","))
}

fn balance_json(balance: &Balance) -> String {
    let optional = |v: Option<f32>| match v {
        Some(value) => num(value),
        None => "null".to_string(),
    };
    let shares = match balance.shares {
        Some(s) => format!(
            "{{{}}}",
            [
                field("genes", &num(s.genes)),
                field("environment", &num(s.environment)),
                field("entangled", &num(s.entangled)),
                field("luck", &num(s.luck)),
            ]
            .join(",")
        ),
        None => "null".to_string(),
    };
    format!(
        "{{{}}}",
        [
            field("sample", &format!("{}", balance.sample)),
            field("shares", &shares),
            field("elasticity", &optional(balance.elasticity)),
            field("siblings", &optional(balance.sibling_correlation)),
            field("mobility", &optional(balance.mobility)),
            field("upbringingGap", &optional(balance.upbringing_gap)),
        ]
        .join(",")
    )
}

// ---- small helpers -----------------------------------------------------------------

fn push(out: &mut String, key: &str, value: &str) {
    out.push_str(&format!("  \"{key}\": {value},\n"));
}

fn field(key: &str, value: &str) -> String {
    format!("\"{key}\":{value}")
}

fn num(value: f32) -> String {
    if !value.is_finite() {
        return "null".to_string();
    }
    // Three places is past anything a viewer can draw, and trailing zeroes are pure
    // weight in a file that carries one number per person per trait.
    let text = format!("{value:.3}");
    let trimmed = text.trim_end_matches('0').trim_end_matches('.');
    // A small negative rounds away to "-0", which is legal JSON but reads as a
    // distinct value to anyone looking at the file. There is one zero.
    match trimmed {
        "" | "-" | "-0" => "0".to_string(),
        other => other.to_string(),
    }
}

/// Strip the leading timestamp a rendered line carries.
fn plain(line: &str) -> String {
    line.split_once("] ")
        .map(|(_, rest)| rest.to_string())
        .unwrap_or_else(|| line.to_string())
}

fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Duration, WorldSeed};

    fn a_world() -> World {
        let mut world = World::genesis(WorldSeed::from_u128(0x11), 30);
        world.record_only(Salience::Pivotal);
        world.run_for(Duration::from_years(20));
        world
    }

    #[test]
    fn a_snapshot_is_well_formed() {
        let world = a_world();
        let json = snapshot(&world, &[], &observer::measure(&world));

        assert!(json.starts_with('{') && json.ends_with('}'));
        assert_eq!(
            json.matches('{').count(),
            json.matches('}').count(),
            "braces should balance"
        );
        assert_eq!(json.matches('[').count(), json.matches(']').count());
        assert!(json.contains("\"places\""));
        assert!(json.contains("\"people\""));
        assert!(json.contains("\"balance\""));
    }

    #[test]
    fn strings_with_awkward_characters_survive() {
        assert_eq!(quoted(r#"a "b" \c"#), r#""a \"b\" \\c""#);
        assert_eq!(quoted("a\nb"), r#""a\nb""#);
        // A real name from the generator, which uses apostrophes and full stops.
        assert_eq!(quoted("Mrs. Marjory O'Kon"), "\"Mrs. Marjory O'Kon\"");
    }

    #[test]
    fn people_reference_each_other_by_index() {
        let world = a_world();
        let json = snapshot(&world, &[], &observer::measure(&world));
        // Somebody should have parents recorded as a pair of indices.
        assert!(
            json.contains("\"parents\":["),
            "no lineage made it into the export"
        );
    }

    #[test]
    fn timestamps_are_stripped_from_event_text() {
        assert_eq!(plain("[yr 100 day 1   06:00] It rained"), "It rained");
        assert_eq!(plain("no stamp here"), "no stamp here");
    }

    #[test]
    fn non_finite_numbers_do_not_produce_invalid_json() {
        assert_eq!(num(f32::NAN), "null");
        assert_eq!(num(f32::INFINITY), "null");
        assert_eq!(num(0.5), "0.5");
        assert_eq!(num(0.0), "0");
        assert_eq!(
            num(-0.0004),
            "0",
            "a rounded-away negative must not read as \"-\""
        );
        assert_eq!(num(0.1234), "0.123");
    }
}
