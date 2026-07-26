//! What an economy has to get right.
//!
//! Mostly these check that the functional form does what a functional form is chosen for.
//! Cobb–Douglas is not decoration: it is picked because neither factor substitutes for the
//! other and because returns to labour on fixed land diminish, and if either of those fails
//! to show up the form is doing no work and a simpler one would be honest.

use super::*;

fn land(fertility: f32, reach: f32, harshness: f32) -> Terrain {
    Terrain {
        fertility,
        reach,
        harshness,
        ..Terrain::middling(0)
    }
}

#[test]
fn nothing_comes_of_nothing() {
    // Hands with no land, and land with no hands. Both make nothing, which is what
    // multiplying rather than adding the two factors means.
    assert_eq!(produce(&land(0.0, 0.5, 0.0), 100.0).output, 0.0);
    assert_eq!(produce(&land(0.8, 0.5, 0.0), 0.0), Ledger::EMPTY);
}

#[test]
fn more_of_either_factor_makes_more() {
    let at = |f, w| produce(&land(f, 0.5, 0.0), w).output;
    let mut last = 0.0;
    for step in 1..20 {
        let now = at(step as f32 / 20.0, 50.0);
        assert!(now >= last, "better land made less");
        last = now;
    }
    let mut last = 0.0;
    for workers in 1..50 {
        let now = at(0.6, workers as f32);
        assert!(now >= last, "more hands made less");
        last = now;
    }
}

#[test]
fn returns_to_labour_diminish() {
    // The Malthusian core. Doubling the workers on fixed land must *not* double output, or
    // crowding costs nothing and a place can absorb the world.
    let one = produce(&land(0.6, 0.5, 0.0), 50.0).output;
    let two = produce(&land(0.6, 0.5, 0.0), 100.0).output;
    assert!(two > one, "more hands should still make more");
    assert!(
        two < one * 2.0,
        "output doubled with the workforce: {one:.1} to {two:.1}"
    );
    // And each extra pair of hands is worth less than the last.
    let per_head = |w| produce(&land(0.6, 0.5, 0.0), w).output / w;
    assert!(per_head(100.0) < per_head(50.0));
}

#[test]
fn crowding_eats_the_surplus() {
    // The consequence that matters: a place can be prosperous, and then be the same place
    // with more people in it and not be. Nothing before this said so.
    let ground = land(0.6, 0.5, 0.0);
    let comfortable = produce(&ground, 20.0);
    let crowded = produce(&ground, 400.0);
    assert!(comfortable.surplus > 0.0, "a small town on good land has spare");
    assert!(
        crowded.per_head() < comfortable.per_head(),
        "crowding left everybody as well off"
    );
    assert!(
        !crowded.self_sufficient(),
        "four hundred people on this land should not feed themselves"
    );
}

#[test]
fn people_eat_before_there_is_a_surplus() {
    let thin = produce(&land(0.05, 0.5, 0.0), 60.0);
    assert!(thin.output > 0.0, "something grows even on poor land");
    assert!(!thin.self_sufficient(), "but not enough for sixty people");
    assert_eq!(thin.prosperity(), 0.0, "a place in deficit is not prosperous");
    assert_eq!(thin.subsistence, 60.0);
}

#[test]
fn a_hard_year_costs_what_a_hard_year_costs() {
    let mild = produce(&land(0.7, 0.5, 0.0), 40.0);
    let brutal = produce(&land(0.7, 0.5, 1.0), 40.0);
    assert!(
        brutal.output < mild.output,
        "a brutal season made as much as a mild one"
    );
    // And it is a cost rather than a catastrophe: the same soil still grows something.
    assert!(brutal.output > 0.0);
}

#[test]
fn a_road_is_worth_having() {
    // Two identical valleys, one reachable and one not. This is the only mechanism in the
    // model that prices a position rather than a soil.
    let ledgers = year(&[
        (land(0.6, 0.95, 0.0), 40.0),
        (land(0.6, 0.05, 0.0), 40.0),
        (land(0.9, 0.60, 0.0), 40.0),
    ]);
    assert_eq!(ledgers[0].surplus, ledgers[1].surplus, "same land, same crop");
    assert!(
        ledgers[0].market > ledgers[1].market,
        "the reachable valley should have more to spend: {:.2} against {:.2}",
        ledgers[0].market,
        ledgers[1].market
    );
    assert!(ledgers[0].prosperity() > ledgers[1].prosperity());
}

#[test]
fn trade_cannot_abolish_geography() {
    // A place with nothing and no road stays with nothing. If trade could feed it, every
    // place would converge and the map would stop mattering.
    let ledgers = year(&[
        (land(0.02, 0.02, 0.9), 80.0),
        (land(0.95, 0.9, 0.0), 30.0),
        (land(0.9, 0.9, 0.0), 30.0),
    ]);
    assert!(!ledgers[0].self_sufficient());
    assert!(
        ledgers[0].per_head() < ledgers[1].per_head() * 0.5,
        "the barren isolated place caught up with the rich one"
    );
}

#[test]
fn a_rich_neighbourhood_lifts_its_neighbours_a_little() {
    let alone = year(&[(land(0.4, 0.7, 0.0), 50.0)]);
    let in_company = year(&[
        (land(0.4, 0.7, 0.0), 50.0),
        (land(0.95, 0.9, 0.0), 25.0),
        (land(0.95, 0.9, 0.0), 25.0),
    ]);
    assert!(
        in_company[0].market > alone[0].market,
        "neighbours with a surplus should be worth something"
    );
    // A little, though. Not enough to make it the same place.
    assert!(in_company[0].market < in_company[1].market);
}

#[test]
fn prosperity_saturates() {
    // The step from nothing spare to a little is enormous; from a lot to more it is not.
    let at = |spare: f32| Ledger {
        market: spare,
        workers: 1.0,
        ..Ledger::EMPTY
    }
    .prosperity();
    assert_eq!(at(0.0), 0.0);
    let early = at(0.9) - at(0.0);
    let late = at(9.0) - at(4.0);
    assert!(early > late * 2.0, "{early:.3} against {late:.3}");
    assert!(at(1000.0) < 1.0, "prosperity should never quite reach one");
}

#[test]
fn an_empty_place_is_vacant_rather_than_destitute() {
    // The same trap the neighbourhood model already fell into once: reading a place off
    // nobody gives zero, which makes an empty place look like the worst slum in the world.
    let nobody = produce(&land(0.9, 0.9, 0.0), 0.0);
    assert_eq!(nobody, Ledger::EMPTY);
    assert_eq!(nobody.per_head(), 0.0, "and no division by zero");
    assert!(nobody.self_sufficient(), "nobody is not starving");
}

#[test]
fn a_region_of_one_place_still_works() {
    let ledgers = year(&[(land(0.6, 0.5, 0.0), 30.0)]);
    assert_eq!(ledgers.len(), 1);
    assert_eq!(ledgers[0].market, ledgers[0].surplus, "nobody to trade with");
    assert!(ledgers[0].market.is_finite());
}

#[test]
fn a_region_of_none_is_not_an_error() {
    assert!(year(&[]).is_empty());
}

#[test]
fn the_numbers_land_where_a_pre_industrial_economy_lands() {
    // The single calibration claim: good land lightly worked leaves a real surplus, and
    // most places most of the time leave very little. A model where everybody is
    // comfortable has not understood the period.
    let good = produce(&land(0.85, 0.6, 0.0), 30.0);
    assert!(
        (0.2..2.0).contains(&good.per_head()),
        "good land left {:.2} a head",
        good.per_head()
    );
    let ordinary = produce(&land(0.45, 0.5, 0.2), 60.0);
    assert!(
        ordinary.per_head() < good.per_head(),
        "ordinary land should not beat good land"
    );
}

// ── the Malthusian check ────────────────────────────────────────────────────────

#[test]
fn an_ordinary_place_is_left_alone_whatever_ordinary_means_here() {
    // The property two earlier attempts got wrong, and the reason they emptied worlds. A
    // feedback has a fixed point; a multiplier below one almost everywhere is a cull. And
    // the fixed point has to be where this world's places actually are, not where a
    // constant says they should be.
    for typical in [0.02, 0.12, 0.24, 0.5, 0.9] {
        assert!(
            (births_relative(typical, typical) - 1.0).abs() < 1e-6,
            "a typical place at {typical} was not left alone"
        );
    }
}

#[test]
fn the_check_cannot_cull_a_world() {
    // Stronger than the last, and the one that actually guarantees it: averaged over any
    // spread of places, centred on that spread's own mean, the multiplier is one. So the
    // check moves births about and never removes them.
    for spread in [
        vec![0.02, 0.05, 0.08, 0.11, 0.14],
        vec![0.1, 0.3, 0.5, 0.7, 0.9],
        vec![0.25; 5],
    ] {
        let typical: f32 = spread.iter().sum::<f32>() / spread.len() as f32;
        let mean: f32 = spread.iter().map(|p| births_relative(*p, typical)).sum::<f32>()
            / spread.len() as f32;
        assert!(
            (mean - 1.0).abs() < 0.02,
            "across {spread:?} centred at {typical:.2} the check averaged {mean:.3}"
        );
    }
}

#[test]
fn plenty_makes_children_and_want_does_not() {
    assert!(births_relative(0.7, 0.24) > 1.0, "a place with food to spare should grow");
    assert!(births_relative(0.02, 0.24) < 1.0, "a place with none should not");
    let mut last = 0.0;
    for step in 0..=20 {
        let now = births_relative(step as f32 / 20.0, 0.24);
        assert!(now >= last, "births fell as the place got richer");
        last = now;
    }
}

#[test]
fn the_check_is_bounded_at_both_ends() {
    // A place with nothing still has some children, and a rich one does not breed without
    // limit. Both bounds are what stop the loop oscillating.
    assert!(births_relative(0.0, 0.5) >= FEWEST);
    assert!(births_relative(1.0, 0.1) <= MOST);
    assert!(births_relative(-5.0, 0.2) >= FEWEST, "and it survives nonsense");
    assert!(births_relative(5.0, 0.2) <= MOST);
}

#[test]
fn the_check_moves_births_towards_the_places_that_can_feed_them() {
    // The point of it. Two places, one with a surplus and one without, should differ in
    // births by enough to matter over a few generations.
    let good = produce(&land(0.85, 0.6, 0.0), 30.0).prosperity();
    let poor = produce(&land(0.2, 0.3, 0.4), 90.0).prosperity();
    let typical = (good + poor) / 2.0;
    let good = births_relative(good, typical);
    let poor = births_relative(poor, typical);
    assert!(
        good > poor * 1.3,
        "good land {good:.2} against poor {poor:.2} — not enough to redistribute anybody"
    );
}

// ── what a people know how to do ────────────────────────────────────────────────

#[test]
fn technique_raises_what_land_yields() {
    let ground = land(0.6, 0.5, 0.0);
    let bare = produce_knowing(&ground, 50.0, Technique::BARE);
    let skilled = produce_knowing(&ground, 50.0, Technique::BARE.after_a_year(5_000.0, 0.8));
    assert!(skilled.output >= bare.output);
    // And `produce` is the bare case, so nothing that predates technique changed meaning.
    assert_eq!(produce(&ground, 50.0), bare);
}

#[test]
fn a_crowd_learns_and_a_handful_forgets() {
    // The Tasmanian result, which is why this is a population variable and not a clock.
    let mut crowd = Technique::BARE;
    let mut few = Technique::BARE;
    for _ in 0..400 {
        crowd = crowd.after_a_year(8_000.0, 0.7);
    }
    assert!(crowd.level() > 1.05, "a large people learned nothing: {}", crowd.level());

    // Now cut the large one off and shrink it, and watch it go.
    let mut stranded = crowd;
    for _ in 0..400 {
        stranded = stranded.after_a_year(300.0, 0.1);
    }
    assert!(
        stranded.level() < crowd.level(),
        "a stranded people kept everything it knew"
    );

    for _ in 0..400 {
        few = few.after_a_year(200.0, 0.2);
    }
    assert_eq!(few, Technique::BARE, "a handful invented something");
}

#[test]
fn nobody_forgets_how_to_eat() {
    let mut destitute = Technique::BARE;
    for _ in 0..5_000 {
        destitute = destitute.after_a_year(1.0, 0.0);
    }
    assert_eq!(destitute.level(), 1.0, "technique fell below bare subsistence");
}

#[test]
fn isolation_is_what_impoverishes() {
    // The same number of people, connected and not. This is the term that makes a road
    // worth having in knowledge as well as in grain.
    let mut connected = Technique::BARE;
    let mut alone = Technique::BARE;
    for _ in 0..600 {
        connected = connected.after_a_year(800.0, 0.95);
        alone = alone.after_a_year(800.0, 0.05);
    }
    assert!(
        connected.level() > alone.level(),
        "connected {:.3} against isolated {:.3}",
        connected.level(),
        alone.level()
    );
}

#[test]
fn learning_slows_as_there_is_more_to_know() {
    let mut level = Technique::BARE;
    let early = {
        let before = level.level();
        for _ in 0..200 {
            level = level.after_a_year(6_000.0, 0.8);
        }
        level.level() - before
    };
    let late = {
        let before = level.level();
        for _ in 0..200 {
            level = level.after_a_year(6_000.0, 0.8);
        }
        level.level() - before
    };
    assert!(late < early, "each technique should be harder than the last");
}

#[test]
fn technique_never_passes_the_pre_industrial_ceiling() {
    let mut level = Technique::BARE;
    for _ in 0..200_000 {
        level = level.after_a_year(100_000.0, 1.0);
    }
    assert!(level.level() <= 3.0 + 1e-6, "{}", level.level());
    assert!(level.level() > 2.5, "it should get most of the way there eventually");
}

#[test]
fn the_malthusian_trap_closes() {
    // The most important consequence, and the reason technique belongs next to the
    // Malthusian check rather than in a crate of its own. Better technique raises what the
    // land yields; the extra food feeds more people; more people on the same land drives
    // the surplus per head back down. Living standards return to where they were and the
    // *population* is what grew.
    //
    // This is the single most robust finding about the ten thousand years before 1800, and
    // here it is arithmetic rather than a claim.
    let ground = land(0.6, 0.5, 0.0);
    let learned = Technique::BARE.after_a_year(1e9, 1.0);
    let better = Technique(3.0);

    // The land now feeds more people at the same standard of living.
    let before_at = |w| produce_knowing(&ground, w, Technique::BARE).per_head();
    let after_at = |w| produce_knowing(&ground, w, better).per_head();
    let _ = learned;

    let standard = before_at(40.0);
    assert!(standard > 0.0);
    assert!(
        after_at(40.0) > standard,
        "before the population responds, technique makes people better off"
    );

    // Find where the improved economy returns to the old standard of living.
    let mut carried = 40.0;
    while after_at(carried) > standard && carried < 100_000.0 {
        carried *= 1.05;
    }
    assert!(
        carried > 40.0 * 2.0,
        "trebling the yield should carry far more people, not slightly more: {carried:.0}"
    );
    assert!(
        (after_at(carried) - standard).abs() < 0.05,
        "and at that size they are no better off than they started"
    );
}

#[test]
fn a_place_that_cannot_feed_itself_says_so() {
    // The reading that was being thrown away. `prosperity` clamps at zero, so it cannot
    // tell a place in famine from one that just breaks even; `want` is the difference.
    let crowded = produce(&land(0.05, 0.5, 0.2), 400.0);
    let barely = produce(&land(0.85, 0.5, 0.2), 1.0);

    assert!(crowded.want() > 0.0, "a place four hundred deep on bad ground is not hungry");
    assert_eq!(
        crowded.prosperity(),
        barely.prosperity().min(crowded.prosperity()),
        "prosperity cannot tell famine from sufficiency, which is why `want` exists",
    );
    assert_eq!(
        produce(&land(0.85, 0.5, 0.2), 1.0).want(),
        0.0,
        "a place with room to spare is hungry",
    );
}

#[test]
fn hunger_deepens_as_a_place_fills_up() {
    // Diminishing returns to labour, felt as hunger. Cobb-Douglas puts output per head on
    // `workers^-0.35`, so past the point the land can carry, every extra pair of hands
    // makes the shortfall worse rather than better.
    let thin = land(0.10, 0.5, 0.2);
    let mut last = -1.0;
    for workers in [50.0, 100.0, 200.0, 400.0, 800.0] {
        let want = produce(&thin, workers).want();
        assert!(
            want > last,
            "{workers} workers wanted {want:.3}, no worse than the {last:.3} before them",
        );
        last = want;
    }
    assert!(last < 1.0, "nobody can be short of more than everything they need");
}

#[test]
fn trade_feeds_a_place_that_cannot_feed_itself() {
    // Which is most of the point of trade, and the reason `want` is measured after it.
    let barren = land(0.06, 0.95, 0.2);
    let alone = produce(&barren, 300.0);
    assert!(alone.want() > 0.0);

    // Neighbours well inside what their ground carries, so they have food to sell.
    let rich = land(0.9, 0.95, 0.1);
    let together = year(&[
        (barren.clone(), 300.0),
        (rich.clone(), 60.0),
        (rich, 60.0),
    ]);
    assert!(
        together[0].want() < alone.want(),
        "neighbours with food to sell did not reduce a hungry place's want",
    );
}

