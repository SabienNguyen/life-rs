//! What the omniscient view has to be able to answer.

use super::dossier::*;
use person::PersonId;
use sim::World;
use sim_core::{Duration, Salience, WorldSeed};

fn a_world() -> World {
    let mut world = World::genesis(WorldSeed::from_u128(0x0b5e), 60);
    world.record_only(Salience::Notable);
    world.run_for(Duration::from_years(70));
    world
}

fn somebody_with_parents(world: &World) -> PersonId {
    world
        .people
        .iter()
        .find(|(_, p)| p.parents.is_some() && p.is_alive())
        .map(|(id, _)| id)
        .expect("nobody was born and lived")
}

#[test]
fn anybody_can_be_asked_about() {
    let world = a_world();
    let mut asked = 0;
    for (id, _) in world.people.iter() {
        let file = dossier(&world, id).expect("a person the world holds has no dossier");
        assert_eq!(file.id, id);
        assert_eq!(file.origins.len(), 5);
        asked += 1;
    }
    assert!(asked > 50, "only {asked} people to ask about");
}

#[test]
fn the_dossier_says_where_and_with_whom() {
    let world = a_world();
    let id = somebody_with_parents(&world);
    let file = dossier(&world, id).unwrap();

    let place = file.place.as_ref().expect("a living person lives nowhere");
    assert!(!place.name.is_empty());
    assert!(place.residents > 0, "their own home has nobody in it");
    assert!((0.0..=1.0).contains(&place.affluence));

    assert!(file.kin.parents.is_some());
    // And a sibling is somebody else's child by the same mother, never themselves.
    assert!(!file.kin.siblings.contains(&id));
}

#[test]
fn every_trait_is_accounted_for_completely() {
    // The claim that makes this worth having: the decomposition is exact, not estimated.
    // Genes plus upbringing plus luck *is* the trait, to the last decimal.
    let world = a_world();
    for (id, _) in world.people.iter() {
        let file = dossier(&world, id).unwrap();
        for share in &file.origins {
            let sum = share.genetic + share.upbringing + share.luck;
            assert!(
                (sum - share.value).abs() < 1e-5,
                "{} came out at {} but its parts add to {sum}",
                share.factor,
                share.value
            );
            assert!(!share.chiefly().is_empty());
        }
    }
}

#[test]
fn a_counterfactual_changes_the_upbringing_and_nothing_else() {
    // "What would she have been like raised somewhere else" is a substitution rather than
    // a re-simulation, because the contributions were never merged in the first place.
    let world = a_world();
    let id = somebody_with_parents(&world);
    let file = dossier(&world, id).unwrap();

    for (i, share) in file.origins.iter().enumerate() {
        let poorer = share.if_raised(-1.5, i);
        let richer = share.if_raised(1.5, i);
        assert!(
            richer > poorer,
            "{} did not respond to where they grew up at all",
            share.factor
        );
        // Whatever changes, what they were born with does not.
        let middle = share.if_raised(0.0, i);
        assert!(
            (middle - (share.genetic + share.luck)).abs() < 1e-5,
            "the counterfactual moved something that was not the upbringing"
        );
    }
}

#[test]
fn a_life_reads_out_of_the_index_and_belongs_to_its_owner() {
    let world = a_world();
    let mut with_history = 0;
    for (id, _) in world.people.iter() {
        let events: Vec<_> = life(&world, id, Salience::Routine).collect();
        if events.is_empty() {
            continue;
        }
        with_history += 1;
        for record in events {
            assert!(record.kind.subjects().contains(&id.to_bits()));
        }
    }
    assert!(with_history > 20, "only {with_history} people had a life");
}

#[test]
fn asking_why_says_what_was_ruled_out_as_well_as_what_was_chosen() {
    // The distinction the whole scoring table exists for. A child does not merely rank
    // work poorly — work is not on the table at all, and those are different facts.
    let world = a_world();
    let child = world
        .people
        .iter()
        .find(|(_, p)| p.is_alive() && p.stage(world.now()).is_dependent())
        .map(|(id, _)| id)
        .expect("nobody young enough");

    let reasoning = why(&world, child).expect("a living person cannot be asked why");
    assert_eq!(reasoning.ranked.len(), person::Deed::COUNT);
    // Ranked best first.
    assert!(reasoning.ranked.windows(2).all(|w| w[0].1 >= w[1].1));
    assert!(
        reasoning.gated.contains(&person::Deed::Work),
        "a child was offered work"
    );
}

#[test]
fn asking_why_does_not_change_what_happens() {
    // The read-only guarantee, tested rather than asserted. Observing a world must not
    // perturb it, and a `why` that re-ran the decision would consume randomness.
    let run = |ask: bool| {
        let mut world = World::genesis(WorldSeed::from_u128(0x7), 20);
        world.record_only(Salience::Pivotal);
        world.run_for(Duration::from_years(3));
        if ask {
            for (id, _) in world.people.iter() {
                let _ = why(&world, id);
                let _ = dossier(&world, id);
            }
        }
        world.run_for(Duration::from_years(3));
        world
            .people
            .iter()
            .map(|(_, p)| (p.name.clone(), p.standing()))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(false), run(true));
}

#[test]
fn the_dead_have_a_dossier_but_no_reasoning() {
    let world = a_world();
    let gone = world
        .people
        .iter()
        .find(|(_, p)| !p.is_alive())
        .map(|(id, _)| id)
        .expect("nobody died");
    assert!(dossier(&world, gone).is_some(), "the dead are unreadable");
    assert!(
        why(&world, gone).is_none(),
        "a corpse was asked what it is thinking"
    );
}

#[test]
fn ancestry_and_descent_walk_the_kinship_graph() {
    let world = a_world();
    let id = somebody_with_parents(&world);

    let up = ancestry(&world, id, 4);
    assert!(!up.is_empty(), "somebody with parents has no ancestry");
    assert_eq!(up[0].len(), 2, "the first generation up is two people");
    // Each generation is at most twice the one before it, and stops at the founders.
    assert!(up.windows(2).all(|w| w[1].len() <= w[0].len() * 2));

    // And descent from a founder should reach somebody.
    let founder = world
        .people
        .iter()
        .find(|(fid, p)| p.parents.is_none() && !world.society.children_of(*fid).is_empty())
        .map(|(id, _)| id)
        .expect("no founder had children");
    let down = descendants(&world, founder, 4);
    assert!(!down.is_empty());
    assert!(down[0].iter().all(|c| {
        world
            .people
            .get(*c)
            .and_then(|p| p.parents)
            .is_some_and(|(m, f)| m == founder || f == founder)
    }));
}

#[test]
fn nobody_is_their_own_ancestor() {
    // The kinship graph has to stay acyclic, and this is the cheapest place to notice
    // that it has not.
    let world = a_world();
    for (id, _) in world.people.iter() {
        for generation in ancestry(&world, id, 6) {
            assert!(!generation.contains(&id), "somebody is their own ancestor");
        }
    }
}
