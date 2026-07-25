//! What evolution has to do for a world to keep having animals in it.

use super::*;
use climate::insolation;
use ecology::Trophic;
use geo::CellId;
use sim_core::{Domain, WorldSeed};

struct World {
    planet: Lithosphere,
    climate: Climate,
    life: Biosphere,
    ecology: Ecology,
    evolution: Evolution,
    rng: Rng,
}

fn a_world(seed: u128, species: usize) -> World {
    let world = WorldSeed::from_u128(seed);
    let mut rng = world.stream(Domain::Evolution, 0, 0);
    let mut planet = Lithosphere::genesis(4, 9, 0.42, &mut rng);
    for _ in 0..25 {
        planet.step_myr(4.0, &mut rng);
    }
    let climate = Climate::genesis(&planet, 4.57, insolation::EARTH_OBLIQUITY);
    let life = Biosphere::read(&planet, &climate);
    let mut ecology = Ecology::genesis(&planet, &life, &climate, species, world);
    for _ in 0..6 {
        ecology.step_myr(&planet, &life, &climate, 1.0, &mut rng);
    }
    let evolution = Evolution::beginning(&ecology);
    World {
        planet,
        climate,
        life,
        ecology,
        evolution,
        rng,
    }
}

impl World {
    fn run(&mut self, steps: usize, dt: f32) {
        for _ in 0..steps {
            self.planet.step_myr(dt, &mut self.rng);
            self.climate.step_myr(&self.planet, dt, &mut self.rng);
            self.life = Biosphere::read(&self.planet, &self.climate);
            self.ecology
                .step_myr(&self.planet, &self.life, &self.climate, dt, &mut self.rng);
            self.evolution.step_myr(
                &self.planet,
                &self.life,
                &self.climate,
                &mut self.ecology,
                dt,
                &mut self.rng,
            );
        }
    }
}

// ---- adaptation ----------------------------------------------------------------------

#[test]
fn a_species_band_follows_the_conditions_it_actually_lives_in() {
    // A species seeded with a band offset from where it can survive should drift towards
    // the middle of what it holds. Nothing selects; the populations in the good part of
    // the range are simply larger, and the mean follows them.
    let mut world = a_world(0x1, 30);
    let id = world.ecology.living().next().expect("nothing alive");

    let centre_of = |ecology: &Ecology, id| {
        let s = ecology.get(id);
        (s.coldest_c + s.warmest_c) / 2.0
    };
    let occupied_mean = |world: &World, id| {
        let mut weight = 0.0f64;
        let mut warmth = 0.0f64;
        for cell in world.planet.grid().cells() {
            let held = world.ecology.biomass_of(id, cell) as f64;
            if held > 0.0 {
                weight += held;
                warmth += held * world.climate.temperature_c(cell) as f64;
            }
        }
        (warmth / weight.max(1.0)) as f32
    };

    let started = centre_of(&world.ecology, id);
    let began_at = occupied_mean(&world, id);
    world.run(30, 1.0);
    if world.ecology.is_extinct(id) {
        return; // Nothing to say about a species that did not survive the test.
    }
    let ended = centre_of(&world.ecology, id);
    let lives_at = occupied_mean(&world, id);

    // Measured against where it lives *now*, not where it lived thirty megayears ago:
    // the planet moved too, and a band that chased the old target would be tracking a
    // world that no longer exists.
    assert!(
        (ended - lives_at).abs() < (started - lives_at).abs() + 0.5,
        "it began at {started:.1} °C living at {began_at:.1}, and ended at {ended:.1} \
         living at {lives_at:.1} — it did not close on anything"
    );
    assert!(
        (ended - lives_at).abs() < 12.0,
        "after thirty megayears its band sits at {ended:.1} °C and it lives at {lives_at:.1}"
    );
}

#[test]
fn adaptation_has_a_speed_limit() {
    // The number that decides whether a climate shift is survivable. A lineage that could
    // re-tune arbitrarily fast would never go extinct from climate at all, which would
    // make the whole of the rest of this pointless.
    let mut world = a_world(0x2, 20);
    let before: Vec<(f32, f32)> = world
        .ecology
        .living()
        .map(|id| {
            let s = world.ecology.get(id);
            (s.coldest_c, s.warmest_c)
        })
        .collect();
    let ids: Vec<_> = world.ecology.living().collect();

    world.evolution.step_myr(
        &world.planet,
        &world.life,
        &world.climate,
        &mut world.ecology,
        1.0,
        &mut world.rng,
    );

    for (i, id) in ids.iter().enumerate() {
        let now = world.ecology.get(*id);
        let moved = (now.coldest_c - before[i].0).abs();
        assert!(
            moved <= TRACKING_C_PER_MYR + 1e-4,
            "a band moved {moved:.2} °C in one megayear"
        );
        // And the width is preserved: adaptation moves a niche, it does not widen one.
        let width_before = before[i].1 - before[i].0;
        assert!((now.warmest_c - now.coldest_c - width_before).abs() < 1e-4);
    }
}

// ---- speciation ------------------------------------------------------------------------

#[test]
fn a_range_broken_in_two_becomes_two_species_given_time() {
    let mut world = a_world(0x3, 40);
    let started = world.ecology.species().len();
    world.run(120, 2.0);
    assert!(
        world.evolution.speciations > 0,
        "not one lineage split in 240 megayears"
    );
    assert!(world.ecology.species().len() > started);
}

#[test]
fn a_daughter_takes_after_its_parent() {
    let mut world = a_world(0x4, 40);
    world.run(120, 2.0);

    let mut checked = 0;
    for id in 0..world.ecology.species().len() as SpeciesId {
        let Some(parent) = world.evolution.parent_of(id) else {
            continue;
        };
        checked += 1;
        let (child, parent) = (world.ecology.get(id), world.ecology.get(parent));
        assert_eq!(child.trophic, parent.trophic, "a grazer begat a predator");
        assert_eq!(child.marine, parent.marine, "a fish begat a land animal");
        assert_eq!(child.mass_kg, parent.mass_kg);
        // Diverged, but not into a different animal — and both have gone on adapting
        // since, so the gap is divergence plus tracking rather than divergence alone.
        let gap = ((child.coldest_c + child.warmest_c) / 2.0
            - (parent.coldest_c + parent.warmest_c) / 2.0)
            .abs();
        assert!(
            gap < 40.0,
            "a daughter diverged {gap:.1} °C from its parent"
        );
    }
    assert!(checked > 0, "nothing had a parent to take after");
}

#[test]
fn splitting_moves_the_range_rather_than_copying_it() {
    // Tested at the moment of the split rather than long after it, because both
    // populations go on spreading and a daughter can perfectly well end up living
    // alongside its parent again. What must never happen is the split itself
    // *manufacturing* animals by leaving the same biomass in two places.
    let mut world = a_world(0x5, 24);
    let id = world.ecology.living().next().expect("nothing alive");
    let grid = world.planet.grid();

    let taken: Vec<CellId> = grid
        .cells()
        .filter(|c| world.ecology.biomass_of(id, *c) > world.ecology.presence_floor(*c))
        .take(6)
        .collect();
    assert!(taken.len() >= 4, "its range is too small to split");

    let before: f32 = grid.cells().map(|c| world.ecology.biomass_of(id, c)).sum();
    let held: Vec<f32> = taken
        .iter()
        .map(|c| world.ecology.biomass_of(id, *c))
        .collect();

    let daughter = world.ecology.get(id).clone();
    let child = world
        .ecology
        .split_off(&taken, id, daughter, &world.life, &world.climate);

    // The parent no longer holds what the daughter took.
    for cell in &taken {
        assert_eq!(world.ecology.biomass_of(id, *cell), 0.0);
    }
    // And the daughter holds exactly it, and nothing else.
    for (cell, was) in taken.iter().zip(&held) {
        assert_eq!(world.ecology.biomass_of(child, *cell), *was);
    }
    let after: f32 = grid
        .cells()
        .map(|c| world.ecology.biomass_of(id, c) + world.ecology.biomass_of(child, c))
        .sum();
    assert!(
        (after - before).abs() < before * 1e-4,
        "the split turned {before:.0} tonnes of animal into {after:.0}"
    );
}

// ---- the tree ----------------------------------------------------------------------------

#[test]
fn the_phylogeny_is_a_tree() {
    let mut world = a_world(0x6, 40);
    world.run(120, 2.0);

    for id in 0..world.ecology.species().len() as SpeciesId {
        let chain = world.evolution.ancestry(id);
        assert!(!chain.contains(&id), "a species is its own ancestor");
        // No repeats: a chain of ancestors is a path, not a graph.
        let mut seen = std::collections::BTreeSet::new();
        for ancestor in &chain {
            assert!(seen.insert(*ancestor), "the ancestry doubles back");
        }
        // A parent is always older than its child.
        if let Some(parent) = world.evolution.parent_of(id) {
            assert!(
                world.evolution.arose(parent) <= world.evolution.arose(id),
                "a species is older than the one it descends from"
            );
        }
    }
}

#[test]
fn descent_and_ancestry_agree_with_each_other() {
    let mut world = a_world(0x7, 40);
    world.run(120, 2.0);
    for id in 0..world.ecology.species().len() as SpeciesId {
        for child in world.evolution.descendants(id) {
            assert!(world.evolution.ancestry(child).contains(&id));
        }
    }
}

// ---- turnover -----------------------------------------------------------------------------

#[test]
fn origination_keeps_pace_with_extinction() {
    // The thing this phase exists for. Without it a fauna only ever thins: Phase 7 on its
    // own loses species steadily and nothing replaces them. With it the count should hold
    // up over hundreds of megayears — not constant, because that would mean nothing is
    // happening, but not collapsing either.
    let mut world = a_world(0x8, 40);
    let started = world.ecology.richness();
    let mut lowest = started;
    for _ in 0..12 {
        world.run(10, 2.0);
        lowest = lowest.min(world.ecology.richness());
    }
    let ended = world.ecology.richness();

    assert!(world.ecology.lost > 0, "nothing died in 240 megayears");
    assert!(world.evolution.speciations > 0, "nothing arose either");
    assert!(
        ended > started / 2,
        "the fauna fell from {started} to {ended} species"
    );
    assert!(lowest > 3, "it very nearly emptied, bottoming at {lowest}");
}

#[test]
fn both_sides_of_the_food_chain_survive_deep_time() {
    let mut world = a_world(0x9, 40);
    world.run(120, 2.0);
    assert!(
        world.ecology.biomass_at_mt(Trophic::Herbivore) > 0.0,
        "the grazers all died"
    );
    assert!(
        world.ecology.biomass_at_mt(Trophic::Carnivore) > 0.0,
        "the predators all died"
    );
}

#[test]
fn the_same_world_evolves_the_same_way() {
    let read = |seed| {
        let mut world = a_world(seed, 30);
        world.run(60, 2.0);
        (
            world.ecology.richness(),
            world.evolution.speciations,
            world.ecology.lost,
        )
    };
    assert_eq!(read(0xABC), read(0xABC));
}
