//! Writing a world out as JSON, so something other than a terminal can show it.
//!
//! Hand-rolled rather than pulling in a serialisation crate: the shape is small, it is
//! wanted in exactly one place, and a viewer is not a reason to put a dependency into
//! the simulation itself.

use observer::Balance;
use person::PersonId;
use society::PlaceId;
use sim::{Happening, World};
use sim_core::Salience;

use crate::render;

/// One year's reading of the world, for the charts.
pub struct YearSample {
    pub year: u64,
    pub living: usize,
    /// Affluence of each place, in arena order.
    pub affluence: Vec<f32>,
    /// The best any place practises, against bare subsistence.
    pub practised: f32,
    /// How far past an ordinary tradition anybody has worked out it is possible to get.
    /// One for a world nobody has had an idea in, which is most of them.
    pub knowledge: f32,
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
    // What the `ways` on a people are, in order, so the viewer does not have to hardcode
    // the deed list to label them.
    let ways: Vec<String> = person::Deed::ALL.iter().map(|d| quoted(d.label())).collect();
    push(&mut out, "wayNames", &format!("[{}]", ways.join(", ")));
    push(&mut out, "planet", &planet(world));
    push(&mut out, "countries", &countries(world));
    push(&mut out, "peoples", &peoples(world));
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
            field("starMass", &num(surface.star().mass_solar as f32)),
            field("starColour", &quoted(surface.star().colour())),
            field("starAgeGyr", &num(surface.star().age_gyr as f32)),
            field("starLeftGyr", &num(surface.star().remaining_gyr() as f32)),
            field("orbitAu", &num(surface.orbit().semi_major_au as f32)),
            field(
                "sunlight",
                &num((surface.orbit().flux(&surface.star()) / cosmos::SOLAR_CONSTANT_WM2) as f32),
            ),
            field("massEarth", &num(surface.orbit().mass_earth as f32)),
            field("gravity", &num(surface.orbit().gravity() as f32)),
            field(
                "yearYears",
                &num(surface.orbit().year_years(&surface.star()) as f32),
            ),
            field("mapWide", &format!("{MAP_WIDE}")),
            field("mapTall", &format!("{MAP_TALL}")),
            field("map", &quoted(&biome_map(surface))),
        ]
        .join(",")
    )
}

/// How wide the map of the founding planet is, in pixels.
///
/// It was a hundred and sixty, which is enough to recognise a continent at a glance and
/// was the whole job when the only reader was a thumbnail with the towns marked. The atlas
/// reads it differently: it wraps the same pixels onto a globe and then magnifies a corner
/// of them until a single cell fills a hand's breadth of screen, so what was once "enough
/// to recognise" has to survive being looked at closely.
///
/// Three hundred and twenty is a quarter-degree of longitude at the equator — still coarser
/// than the level-three grid underneath it, so no detail is being invented — and costs
/// about fifty kilobytes in the page, which compresses to almost nothing because a map is
/// mostly runs of the same biome.
const MAP_WIDE: usize = 320;
const MAP_TALL: usize = MAP_WIDE / 2;

/// The planet's biomes, one character a pixel, row-major from the north pole.
///
/// A character rather than base64 because the alphabet is fifteen long: one biome index
/// per pixel written as a printable byte reads straight out of a string in the viewer
/// with no decoding at all, and gzips to nothing.
fn biome_map(surface: &sim::Surface) -> String {
    let grid = surface.planet.grid();
    let mut out = String::with_capacity(MAP_WIDE * MAP_TALL);
    // Scanline order with the previous pixel as the search hint: neighbouring pixels are
    // neighbouring places, so each lookup finishes in a hop or two.
    let mut hint = 0u32;
    for row in 0..MAP_TALL {
        let latitude = (90.0 - 180.0 * (row as f64 + 0.5) / MAP_TALL as f64).to_radians();
        for column in 0..MAP_WIDE {
            let longitude = (-180.0 + 360.0 * (column as f64 + 0.5) / MAP_WIDE as f64).to_radians();
            let direction = geo::Vec3::new(
                latitude.cos() * longitude.cos(),
                latitude.cos() * longitude.sin(),
                latitude.sin(),
            );
            hint = grid.nearest_to(direction, hint);
            out.push((b'A' + surface.life.biome(hint) as u8) as char);
        }
    }
    out
}

/// The countries, largest first, each naming the places in it.
///
/// Places are given by name rather than index, because the roster a country is expressed in
/// is `culture`'s own numbering and includes places that have since emptied — so an index
/// here would not line up with the `places` array and would be a trap for the viewer.
fn countries(world: &World) -> String {
    let entries: Vec<String> = world
        .countries()
        .iter()
        .map(|country| {
            let souls: u32 = country
                .places
                .iter()
                .filter_map(|at| world.souls_at(*at))
                .sum();
            let within: Vec<String> = country
                .places
                .iter()
                .filter_map(|at| world.place_named(*at))
                .map(quoted)
                .collect();
            format!(
                "{{{}}}",
                [
                    field("name", &quoted(&country.name)),
                    field(
                        "people",
                        &quoted(
                            world
                                .peoples()
                                .get(country.culture)
                                .map(|p| p.name.as_str())
                                .unwrap_or("")
                        )
                    ),
                    field("souls", &format!("{souls}")),
                    format!("\"places\": [{}]", within.join(", ")),
                ]
                .join(", ")
            )
        })
        .collect();
    format!("[{}]", entries.join(", "))
}

/// Every people who still have anybody practising them, with their descent.
///
/// `from` is a name rather than an index for the same reason, and because a people's parent
/// may itself be extinct — the line of descent is the interesting part and it has to survive
/// the death of everybody on it.
fn peoples(world: &World) -> String {
    let all = world.peoples();
    let entries: Vec<String> = all
        .iter()
        .filter(|people| people.living())
        .map(|people| {
            let ways: Vec<String> = people.ways.iter().map(|w| num(*w)).collect();
            format!(
                "{{{}}}",
                [
                    field("name", &quoted(&people.name)),
                    field("souls", &format!("{}", people.souls)),
                    field(
                        "from",
                        &match people.parent.and_then(|of| all.get(of)) {
                            Some(parent) => quoted(&parent.name),
                            None => "null".to_string(),
                        }
                    ),
                    field("arose", &format!("{}", people.arose)),
                    format!("\"ways\": [{}]", ways.join(", ")),
                ]
                .join(", ")
            )
        })
        .collect();
    format!("[{}]", entries.join(", "))
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
                    // How far short of feeding itself the place fell, per head. Zero for
                    // a place that manages, which is the usual case and the interesting
                    // exception.
                    field("want", &num(place.want)),
                    // What a head gets out of the ground here this year, and what the place
                    // has been like for a generation. The second is what decides where
                    // anybody moves — see §30.5 — so it belongs in a view of the place.
                    field("prosperity", &num(place.prosperity)),
                    field("fortune", &num(place.fortune)),
                    // What the place owns that outlives the year — the only capital in this
                    // world, and the only thing in it that compounds.
                    field(
                        "tools",
                        &num(economy::all_tools(&world.holdings_of(id)))
                    ),
                    // What its people actually know how to do, and how far anybody has ever
                    // worked out that it is possible to get. One is bare subsistence and one
                    // is an age nobody has left.
                    field("practised", &num(world.technique_of(id).level())),
                    field("knowledge", &num(world.technique_of(id).reach_of_knowledge())),
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
                    // Where to draw it on the little map, as a fraction across and down.
                    // Computed here rather than in the page because the projection is the
                    // map's business and the map is written here.
                    field(
                        "atX",
                        &place.terrain.as_ref().map_or("null".to_string(), |t| {
                            num((t.longitude + 180.0) / 360.0)
                        }),
                    ),
                    field(
                        "atY",
                        &place
                            .terrain
                            .as_ref()
                            .map_or("null".to_string(), |t| num((90.0 - t.latitude) / 180.0)),
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

    // Everybody's position, read once per place rather than once per person: a position is
    // a rank against the neighbours, so asking for one person's costs the whole town's.
    let mut positions: std::collections::BTreeMap<PersonId, (bonds::Role, String)> =
        Default::default();
    for place in world.places.ids() {
        let ways = world
            .people_of(place)
            .map(|people| people.ways)
            .unwrap_or([0.5; culture::WAYS]);
        for (who, _, role) in world.society_of(place) {
            positions.insert(who, (role, culture::naming::name_a_role(&ways, role.stem())));
        }
    }

    let entries: Vec<String> = world
        .people
        .iter()
        .map(|(id, p)| {
            let now = world.now();
            // Whether they are anything for a living. A child is not, and the dead still are:
            // reading "trade: —" under a life whose record says she gave up cooking for
            // farming twice is the view contradicting itself.
            let working = if p.is_alive() {
                !p.stage(now).is_dependent()
            } else {
                p.has_matured()
            };
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
                    field(
                        "country",
                        &quoted(
                            &world
                                .country_of(id)
                                .unwrap_or_else(|| "nowhere in particular".to_string()),
                        ),
                    ),
                    field("standing", &num(p.peak_standing())),
                    field(
                        "title",
                        &match positions.get(&id) {
                            Some((_, word)) => quoted(word),
                            None => "null".to_string(),
                        }
                    ),
                    field(
                        "role",
                        &match positions.get(&id) {
                            Some((role, _)) => quoted(role.label()),
                            None => "null".to_string(),
                        }
                    ),
                    field(
                        "allies",
                        &format!(
                            "{}",
                            world.bonds.of(id).filter(|(_, t)| t.allied()).count()
                        )
                    ),
                    field(
                        "owed",
                        &num(world.bonds.of(id).map(|(_, t)| t.debt).sum::<f32>())
                    ),
                    field("repute", &num(world.repute_of(id))),
                    // A child is not anything for a living. `Person` carries a trade from
                    // birth because everybody starts a farmer, but nobody is one until they
                    // are old enough to be counted as a hand.
                    // The dead keep the trade and lose the word for it: `trade_of` names it in
                    // the local people's own speech, and that needs a place, which somebody
                    // who has died no longer has. What they did is still known.
                    field(
                        "trade",
                        &if working {
                            quoted(p.trade().label())
                        } else {
                            "null".to_string()
                        }
                    ),
                    field(
                        "craft",
                        &match working.then(|| world.trade_of(id)).flatten() {
                            Some((_, word)) => quoted(&word),
                            None => "null".to_string(),
                        }
                    ),
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
    // Which slot in `people` each identifier belongs to, so an event can say who it is about
    // and a reader can ask for one life. A biography is the chronicle filtered by participant
    // — that is what the chronicle is *for* — and until now nothing displayed it.
    let slot: std::collections::HashMap<PersonId, usize> =
        world.people.ids().enumerate().map(|(at, id)| (id, at)).collect();
    let acre: std::collections::HashMap<PlaceId, usize> =
        world.places.ids().enumerate().map(|(at, id)| (id, at)).collect();
    let entries: Vec<String> = world
        .chronicle
        .at_least(Salience::Pivotal)
        .map(|record| {
            let year = calendar.map(|c| c.date_at(record.at).year).unwrap_or(0);
            // What kind of thing it was, and everybody it concerns — one question, because
            // both are read off the same shape. A birth is about three people and being
            // taken up is about two, so each of those lives carries the event.
            //
            // Deliberately not `subjects()`: that is the chronicle's own index, and its
            // handles are erased to bare bits, where a place and a person of the same age
            // and slot are the same number. Filing by it would post somebody's move into
            // a stranger's life. Matching here keeps the handles typed.
            //
            // The same for the places an event names, so a settlement has a history for the
            // same reason a person has a life. Only what the happening actually says: a move
            // names where it went, a change of character names the place it happened to.
            // Where somebody was standing when they were born is not in the record and is
            // not guessed at here.
            let (kind, folk, ground): (&str, Vec<PersonId>, Vec<PlaceId>) = match record.kind {
                Happening::WorldBegins { .. } => ("world", vec![], vec![]),
                Happening::PersonBorn {
                    child,
                    mother,
                    father,
                } => ("birth", vec![child, mother, father], vec![]),
                Happening::PersonDies { person, .. } => ("death", vec![person], vec![]),
                Happening::PersonPairs { person, with } => ("pairing", vec![person, with], vec![]),
                Happening::PersonMoves { person, to } => ("move", vec![person], vec![to]),
                Happening::PersonMentored { person, by } => ("patron", vec![person, by], vec![]),
                Happening::PlaceChanges { place, .. } => ("place", vec![], vec![place]),
                // The rarest thing in the chronicle and the only one that changes what is
                // *possible* rather than what happened.
                Happening::PersonWorksItOut { person, .. } => ("advance", vec![person], vec![]),
                Happening::PersonRetrains { person, .. } => ("trade", vec![person], vec![]),
                Happening::PersonArrives { person } => ("other", vec![person], vec![]),
                Happening::PersonDoes { person, .. } => ("other", vec![person], vec![]),
                _ => ("other", vec![], vec![]),
            };
            let who: Vec<String> = folk
                .iter()
                .filter_map(|id| slot.get(id))
                .map(|at| at.to_string())
                .collect();
            let here: Vec<String> = ground
                .iter()
                .filter_map(|id| acre.get(id))
                .map(|at: &usize| at.to_string())
                .collect();
            format!(
                "{{{}}}",
                [
                    field("year", &format!("{year}")),
                    field("kind", &quoted(kind)),
                    field("who", &format!("[{}]", who.join(","))),
                    field("here", &format!("[{}]", here.join(","))),
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
                    field("practised", &num(s.practised)),
                    field("knowledge", &num(s.knowledge)),
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

    #[test]
    fn the_peoples_and_countries_go_out_with_the_world() {
        let world = a_world();
        let json = snapshot(&world, &[], &observer::measure(&world));
        assert!(json.contains("\"countries\":"), "no countries in the export");
        assert!(json.contains("\"peoples\":"), "no peoples in the export");
        // The ways need their labels alongside them or the array is seven bare numbers.
        assert!(json.contains("\"wayNames\":"), "no way names in the export");
        for deed in person::Deed::ALL {
            assert!(json.contains(deed.label()), "no label for {}", deed.label());
        }

        // Every country names a people that is actually in the export, and places that
        // are actually in the world — indices would not survive the roster keeping
        // emptied places, so these are names and they have to resolve.
        for country in world.countries() {
            assert!(json.contains(&country.name), "a country missing from the export");
            for at in &country.places {
                let named = world.place_named(*at).expect("a country holds a real place");
                assert!(
                    world.places.iter().any(|(_, p)| p.name == named),
                    "country names a place {named} that is not in this world",
                );
            }
        }
    }

    #[test]
    fn nobody_in_the_export_is_from_a_country_that_is_not_in_it() {
        // The property the deleted enum could never have had: a person's country is one of
        // the countries this world actually has, because it was looked up from where they
        // live rather than carried around.
        let world = a_world();
        let json = snapshot(&world, &[], &observer::measure(&world));
        let named: Vec<String> = world.countries().into_iter().map(|c| c.name).collect();

        let mut checked = 0;
        for (id, person) in world.people.iter() {
            if !person.is_alive() {
                continue;
            }
            if let Some(from) = world.country_of(id) {
                assert!(named.contains(&from), "somebody is from {from}, which does not exist");
                assert!(json.contains(&from));
                checked += 1;
            }
        }
        assert!(checked > 0, "nobody in this world is from anywhere");
    }

    #[test]
    fn the_planet_and_its_map_go_out_with_the_world() {
        let world = a_world();
        let json = snapshot(&world, &[], &observer::measure(&world));
        assert!(json.contains("\"planet\":"), "no planet in the export");
        assert!(json.contains("\"map\":"), "no map in the export");

        // Every quarter carries a place on that map, and it is inside it.
        for place in world.places.iter().filter_map(|(_, p)| p.terrain.as_ref()) {
            let x = (place.longitude + 180.0) / 360.0;
            let y = (90.0 - place.latitude) / 180.0;
            assert!((0.0..=1.0).contains(&x), "a quarter at {x} across the map");
            assert!((0.0..=1.0).contains(&y), "a quarter at {y} down the map");
        }
    }

    #[test]
    fn the_map_is_one_readable_character_per_pixel() {
        let world = a_world();
        let surface = world.surface().expect("a founded world has ground");
        let map = biome_map(surface);
        assert_eq!(map.len(), MAP_WIDE * MAP_TALL);
        for byte in map.bytes() {
            // Printable, inside the alphabet, and a biome that exists — the viewer
            // subtracts 'A' and indexes a table of fifteen.
            let index = byte - b'A';
            assert!(
                (index as usize) < biome::Biome::COUNT,
                "the map claimed biome {index}"
            );
        }
        // A planet is not one biome from pole to pole.
        let distinct: std::collections::BTreeSet<u8> = map.bytes().collect();
        assert!(distinct.len() > 4, "only {} biomes on the map", distinct.len());
        // And it needs no escaping, which is why it is written as a bare string.
        assert!(!map.contains('"') && !map.contains('\\'));
    }
}
