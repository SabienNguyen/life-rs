//! Households, partnerships, and kinship.
//!
//! Kinship is stored **structurally** — parent edges only. Siblings, cousins, ancestors
//! and descendants are derived by traversal rather than recorded, because storing them
//! explodes the edge count and, worse, lets the copies drift out of agreement with each
//! other. A relationship you can compute is one that cannot become wrong.
//!
//! Everything here uses ordered maps. Hash iteration order varies between runs, and any
//! system that iterates one would quietly break the promise that a seed replays exactly.

use person::PersonId;
use sim_core::{Arena, Id, Time};
use std::collections::{BTreeMap, BTreeSet};

pub type HouseholdId = Id<Household>;

/// People who live together and raise children together.
#[derive(Clone, Debug, PartialEq)]
pub struct Household {
    pub members: Vec<PersonId>,
    pub founded: Time,
    /// This household's contribution to the personality of everyone raised in it.
    ///
    /// The shared-environment term. It is what makes siblings resemble each other
    /// beyond their genes, and it is drawn once per household rather than per child —
    /// which is precisely what "shared" means.
    pub upbringing: f32,
}

impl Household {
    pub fn contains(&self, person: PersonId) -> bool {
        self.members.contains(&person)
    }

    pub fn size(&self) -> usize {
        self.members.len()
    }
}

/// The social structure of a world.
#[derive(Default)]
pub struct Society {
    households: Arena<Household>,
    parents: BTreeMap<PersonId, (PersonId, PersonId)>,
    children: BTreeMap<PersonId, Vec<PersonId>>,
    partners: BTreeMap<PersonId, PersonId>,
    lives_in: BTreeMap<PersonId, HouseholdId>,
}

impl Society {
    pub fn new() -> Society {
        Society::default()
    }

    // ---- households ----------------------------------------------------------

    pub fn found_household(&mut self, founded: Time, upbringing: f32) -> HouseholdId {
        self.households.insert(Household {
            members: Vec::new(),
            founded,
            upbringing,
        })
    }

    pub fn household(&self, id: HouseholdId) -> Option<&Household> {
        self.households.get(id)
    }

    pub fn households(&self) -> impl Iterator<Item = (HouseholdId, &Household)> {
        self.households.iter()
    }

    pub fn household_count(&self) -> usize {
        self.households.len()
    }

    pub fn home_of(&self, person: PersonId) -> Option<HouseholdId> {
        self.lives_in.get(&person).copied()
    }

    /// The upbringing a child raised here would receive. Zero if they have no home.
    pub fn upbringing_in(&self, home: Option<HouseholdId>) -> f32 {
        home.and_then(|id| self.households.get(id))
            .map(|h| h.upbringing)
            .unwrap_or(0.0)
    }

    pub fn move_in(&mut self, person: PersonId, home: HouseholdId) {
        self.move_out(person);
        if let Some(household) = self.households.get_mut(home) {
            household.members.push(person);
            self.lives_in.insert(person, home);
        }
    }

    pub fn move_out(&mut self, person: PersonId) {
        if let Some(previous) = self.lives_in.remove(&person)
            && let Some(household) = self.households.get_mut(previous)
        {
            household.members.retain(|m| *m != person);
        }
    }

    /// Remove households nobody lives in any more.
    pub fn dissolve_empty(&mut self) -> usize {
        let empty: Vec<HouseholdId> = self
            .households
            .iter()
            .filter(|(_, h)| h.members.is_empty())
            .map(|(id, _)| id)
            .collect();
        for id in &empty {
            self.households.remove(*id);
        }
        empty.len()
    }

    // ---- partnerships --------------------------------------------------------

    pub fn pair(&mut self, a: PersonId, b: PersonId) {
        debug_assert_ne!(a, b, "nobody partners with themselves");
        self.separate(a);
        self.separate(b);
        self.partners.insert(a, b);
        self.partners.insert(b, a);
    }

    pub fn separate(&mut self, person: PersonId) {
        if let Some(other) = self.partners.remove(&person) {
            self.partners.remove(&other);
        }
    }

    pub fn partner_of(&self, person: PersonId) -> Option<PersonId> {
        self.partners.get(&person).copied()
    }

    pub fn is_partnered(&self, person: PersonId) -> bool {
        self.partners.contains_key(&person)
    }

    // ---- kinship -------------------------------------------------------------

    pub fn record_birth(&mut self, child: PersonId, mother: PersonId, father: PersonId) {
        self.parents.insert(child, (mother, father));
        self.children.entry(mother).or_default().push(child);
        self.children.entry(father).or_default().push(child);
    }

    pub fn parents_of(&self, child: PersonId) -> Option<(PersonId, PersonId)> {
        self.parents.get(&child).copied()
    }

    pub fn children_of(&self, person: PersonId) -> &[PersonId] {
        self.children.get(&person).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Everyone sharing at least one parent. Derived, never stored.
    pub fn siblings_of(&self, person: PersonId) -> Vec<PersonId> {
        let Some((mother, father)) = self.parents_of(person) else {
            return Vec::new();
        };
        let mut found: BTreeSet<PersonId> = BTreeSet::new();
        for parent in [mother, father] {
            found.extend(self.children_of(parent).iter().copied());
        }
        found.remove(&person);
        found.into_iter().collect()
    }

    /// Ancestors up to `generations` back, nearest first.
    pub fn ancestors_of(&self, person: PersonId, generations: u8) -> Vec<PersonId> {
        let mut found = Vec::new();
        let mut seen = BTreeSet::new();
        let mut frontier = vec![person];

        for _ in 0..generations {
            let mut next = Vec::new();
            for id in frontier {
                if let Some((mother, father)) = self.parents_of(id) {
                    for parent in [mother, father] {
                        if seen.insert(parent) {
                            found.push(parent);
                            next.push(parent);
                        }
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
        found
    }

    /// Everyone descended from this person.
    pub fn descendants_of(&self, person: PersonId) -> Vec<PersonId> {
        let mut found = Vec::new();
        let mut seen = BTreeSet::new();
        let mut frontier = vec![person];

        while let Some(id) = frontier.pop() {
            for child in self.children_of(id) {
                if seen.insert(*child) {
                    found.push(*child);
                    frontier.push(*child);
                }
            }
        }
        found.sort();
        found
    }

    pub fn is_ancestor_of(&self, elder: PersonId, younger: PersonId) -> bool {
        let mut frontier = vec![younger];
        let mut seen = BTreeSet::new();
        while let Some(id) = frontier.pop() {
            if let Some((mother, father)) = self.parents_of(id) {
                for parent in [mother, father] {
                    if parent == elder {
                        return true;
                    }
                    if seen.insert(parent) {
                        frontier.push(parent);
                    }
                }
            }
        }
        false
    }

    /// Close enough that pairing should be ruled out: siblings, parent and child,
    /// grandparent and grandchild, aunt or uncle and niece or nephew.
    pub fn is_close_kin(&self, a: PersonId, b: PersonId) -> bool {
        if a == b {
            return true;
        }
        if self.siblings_of(a).contains(&b) {
            return true;
        }
        // Two generations covers parent, grandparent, and — via shared ancestors —
        // aunts, uncles and first cousins.
        let ancestors_a: BTreeSet<PersonId> =
            self.ancestors_of(a, 2).into_iter().chain([a]).collect();
        let ancestors_b: BTreeSet<PersonId> =
            self.ancestors_of(b, 2).into_iter().chain([b]).collect();
        ancestors_a.intersection(&ancestors_b).next().is_some()
    }

    /// Everyone whose parentage is on record. Useful for auditing a run.
    pub fn known_children(&self) -> usize {
        self.parents.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use person::Person;
    use sim_core::{Domain, WorldSeed};

    /// A population of real people, so the handles under test are real handles.
    fn population(n: usize) -> (Arena<Person>, Vec<PersonId>) {
        let mut planets = Arena::new();
        let home = planets.insert(planet_stub());
        let mut people = Arena::new();
        let pool = genetics_pool();
        let ids = (0..n)
            .map(|i| {
                let mut rng = WorldSeed::from_u128(0x50c1e7).stream(Domain::Genetics, i as u64, 0);
                people.insert(person::found(
                    genetics::standard_architecture(),
                    &pool,
                    &mut rng,
                    home,
                    Time::ORIGIN,
                    0.0,
                ))
            })
            .collect();
        (people, ids)
    }

    fn planet_stub() -> planet::Planet {
        planet::Planet::earth()
    }

    fn genetics_pool() -> genetics::FounderPool {
        genetics::FounderPool::uniform()
    }

    #[test]
    fn households_gain_and_lose_members() {
        let (_people, ids) = population(3);
        let mut society = Society::new();
        let home = society.found_household(Time::ORIGIN, 0.5);

        society.move_in(ids[0], home);
        society.move_in(ids[1], home);
        assert_eq!(society.household(home).unwrap().size(), 2);
        assert_eq!(society.home_of(ids[0]), Some(home));
        assert!(society.household(home).unwrap().contains(ids[1]));

        society.move_out(ids[0]);
        assert_eq!(society.household(home).unwrap().size(), 1);
        assert_eq!(society.home_of(ids[0]), None);
    }

    #[test]
    fn moving_house_does_not_leave_someone_in_two_places() {
        let (_people, ids) = population(1);
        let mut society = Society::new();
        let first = society.found_household(Time::ORIGIN, 0.0);
        let second = society.found_household(Time::ORIGIN, 0.0);

        society.move_in(ids[0], first);
        society.move_in(ids[0], second);

        assert_eq!(society.household(first).unwrap().size(), 0);
        assert_eq!(society.household(second).unwrap().size(), 1);
        assert_eq!(society.home_of(ids[0]), Some(second));
    }

    #[test]
    fn empty_households_are_dissolved() {
        let (_people, ids) = population(1);
        let mut society = Society::new();
        let lived_in = society.found_household(Time::ORIGIN, 0.0);
        society.found_household(Time::ORIGIN, 0.0);
        society.move_in(ids[0], lived_in);

        assert_eq!(society.dissolve_empty(), 1);
        assert_eq!(society.household_count(), 1);
        assert!(society.household(lived_in).is_some());
    }

    #[test]
    fn partnerships_are_symmetric_and_exclusive() {
        let (_people, ids) = population(3);
        let mut society = Society::new();

        society.pair(ids[0], ids[1]);
        assert_eq!(society.partner_of(ids[0]), Some(ids[1]));
        assert_eq!(society.partner_of(ids[1]), Some(ids[0]));

        // Taking a new partner must free the old one, not leave a dangling half-edge.
        society.pair(ids[1], ids[2]);
        assert_eq!(society.partner_of(ids[0]), None);
        assert_eq!(society.partner_of(ids[2]), Some(ids[1]));

        society.separate(ids[1]);
        assert!(!society.is_partnered(ids[1]));
        assert!(!society.is_partnered(ids[2]));
    }

    #[test]
    fn siblings_are_derived_not_stored() {
        let (_people, ids) = population(6);
        let (mother, father) = (ids[0], ids[1]);
        let mut society = Society::new();

        society.record_birth(ids[2], mother, father);
        society.record_birth(ids[3], mother, father);
        society.record_birth(ids[4], mother, father);

        let siblings = society.siblings_of(ids[2]);
        assert_eq!(siblings.len(), 2);
        assert!(siblings.contains(&ids[3]) && siblings.contains(&ids[4]));
        assert!(!siblings.contains(&ids[2]), "nobody is their own sibling");

        // An only child has none, and neither does someone with no recorded parents.
        assert!(society.siblings_of(ids[5]).is_empty());
    }

    #[test]
    fn half_siblings_count_as_siblings() {
        let (_people, ids) = population(5);
        let mut society = Society::new();
        society.record_birth(ids[3], ids[0], ids[1]);
        society.record_birth(ids[4], ids[0], ids[2]); // same mother, different father

        assert!(society.siblings_of(ids[3]).contains(&ids[4]));
    }

    #[test]
    fn lineage_runs_both_ways() {
        // Four generations down one line.
        let (_people, ids) = population(9);
        let mut society = Society::new();
        society.record_birth(ids[2], ids[0], ids[1]); // child of the founders
        society.record_birth(ids[5], ids[2], ids[3]); // grandchild
        society.record_birth(ids[7], ids[5], ids[6]); // great-grandchild

        let ancestors = society.ancestors_of(ids[7], 3);
        assert!(ancestors.contains(&ids[5]), "parent");
        assert!(ancestors.contains(&ids[2]), "grandparent");
        assert!(ancestors.contains(&ids[0]), "great-grandparent");

        // Depth is respected.
        assert!(!society.ancestors_of(ids[7], 1).contains(&ids[2]));

        let descendants = society.descendants_of(ids[0]);
        assert!(descendants.contains(&ids[2]));
        assert!(descendants.contains(&ids[5]));
        assert!(descendants.contains(&ids[7]));
        assert!(!descendants.contains(&ids[0]));

        assert!(society.is_ancestor_of(ids[0], ids[7]));
        assert!(!society.is_ancestor_of(ids[7], ids[0]));
    }

    #[test]
    fn close_kin_covers_the_relationships_pairing_must_avoid() {
        let (_people, ids) = population(10);
        let mut society = Society::new();
        let (grandmother, grandfather) = (ids[0], ids[1]);
        society.record_birth(ids[2], grandmother, grandfather);
        society.record_birth(ids[3], grandmother, grandfather); // sibling of ids[2]
        society.record_birth(ids[5], ids[2], ids[4]); // child of ids[2]
        society.record_birth(ids[7], ids[3], ids[6]); // cousin of ids[5]

        assert!(society.is_close_kin(ids[2], ids[3]), "siblings");
        assert!(society.is_close_kin(ids[2], ids[5]), "parent and child");
        assert!(society.is_close_kin(grandmother, ids[5]), "grandparent");
        assert!(society.is_close_kin(ids[3], ids[5]), "aunt and nephew");
        assert!(society.is_close_kin(ids[5], ids[7]), "first cousins");
        assert!(society.is_close_kin(ids[5], ids[5]), "themselves");

        // Two unrelated people are not.
        assert!(!society.is_close_kin(ids[8], ids[9]));
        assert!(!society.is_close_kin(ids[4], ids[6]), "the two in-laws");
    }

    #[test]
    fn kinship_traversal_terminates_on_a_cycle() {
        // Should be impossible, but a malformed pedigree must not hang the simulation.
        let (_people, ids) = population(2);
        let mut society = Society::new();
        society.record_birth(ids[0], ids[1], ids[1]);
        society.record_birth(ids[1], ids[0], ids[0]);

        assert!(society.ancestors_of(ids[0], 10).len() <= 2);
        assert!(society.descendants_of(ids[0]).len() <= 2);
        assert!(society.is_ancestor_of(ids[1], ids[0]));
    }

    #[test]
    fn upbringing_is_a_property_of_the_household_not_the_child() {
        let (_people, ids) = population(2);
        let mut society = Society::new();
        let home = society.found_household(Time::ORIGIN, 0.75);
        society.move_in(ids[0], home);
        society.move_in(ids[1], home);

        // Two children raised together receive the same shared term — which is what
        // makes them resemble each other beyond their genes.
        assert_eq!(society.upbringing_in(society.home_of(ids[0])), 0.75);
        assert_eq!(society.upbringing_in(society.home_of(ids[1])), 0.75);
        assert_eq!(society.upbringing_in(None), 0.0, "no home, no shared term");
    }
}
