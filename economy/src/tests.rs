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
