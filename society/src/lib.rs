//! Households, partnerships, and kinship.
//!
//! Kinship is stored **structurally** — parent edges only. Siblings, cousins, ancestors
//! and descendants are derived by traversal rather than recorded, because storing them
//! explodes the edge count and, worse, lets the copies drift out of agreement with each
//! other. A relationship you can compute is one that cannot become wrong.
//!
//! Everything here uses ordered maps. Hash iteration order varies between runs, and any
//! system that iterates one would quietly break the promise that a seed replays exactly.

pub mod place;
pub mod terrain;

use person::PersonId;
pub use place::{Archetype, Census, EnvironmentVector, Place, PlaceId};
pub use terrain::Terrain;
use sim_core::{Arena, Id, Time};
use std::collections::{BTreeMap, BTreeSet};

pub type HouseholdId = Id<Household>;

/// People who live together and raise children together.
#[derive(Clone, Debug, PartialEq)]
pub struct Household {
    pub members: Vec<PersonId>,
    /// Which neighbourhood they live in.
    pub place: Option<PlaceId>,
    pub founded: Time,
    /// This household's contribution to the personality of everyone raised in it.
    ///
    /// The shared-environment term. It is what makes siblings resemble each other
    /// beyond their genes, and it is drawn once per household rather than per child —
    /// which is precisely what "shared" means.
    pub upbringing: f32,
}

impl Household {
    /// Whoever speaks for this household, or nobody if it holds no grown adult.
    ///
    /// **A reading, never a fact.** Nobody is appointed and nothing is stored, so the
    /// household cannot come to have a head who has died, or two, or none while an adult
    /// stands in it — the three ways a stored one rots. It is the same discipline §26 applies
    /// to a village's elders and for the same reason.
    ///
    /// Succession is then not a mechanism at all. When the head dies the reading simply
    /// returns somebody else, on the next question anybody asks, without an event or a rule
    /// about who inherits what. A household whose standing halves the year its earner dies
    /// did not have a succession crisis written for it; it had one because that is what the
    /// arithmetic says when you ask again.
    ///
    /// Standing first and age only to break a tie: what a household can put behind a claim is
    /// what its strongest member can, and seniority decides between equals.
    pub fn head(&self, of: impl Fn(PersonId) -> Option<(f32, f64)>) -> Option<PersonId> {
        self.members
            .iter()
            .filter_map(|id| of(*id).map(|(standing, age)| (*id, standing, age)))
            .max_by(|a, b| a.1.total_cmp(&b.1).then(a.2.total_cmp(&b.2)))
            .map(|(id, _, _)| id)
    }

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
            place: None,
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

    /// Which neighbourhood someone lives in, if any.
    pub fn place_of(&self, person: PersonId) -> Option<PlaceId> {
        self.households.get(self.home_of(person)?)?.place
    }

    /// Move a household to a neighbourhood. Returns the place it left.
    pub fn settle(&mut self, home: HouseholdId, place: PlaceId) -> Option<PlaceId> {
        let household = self.households.get_mut(home)?;
        let previous = household.place.replace(place);
        previous.filter(|old| *old != place)
    }

    pub fn households_in(&self, place: PlaceId) -> impl Iterator<Item = (HouseholdId, &Household)> {
        self.households
            .iter()
            .filter(move |(_, h)| h.place == Some(place))
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

    #[test]
    fn a_household_has_a_head_and_it_is_read_rather_than_held() {
        // §26.9 asked for households to be political units. A head is the first part of
        // that, and it is a *reading* for the same reason a village's elders are: a stored
        // one can be dead, or absent while an adult stands in the room, or two at once, and
        // none of those can happen to a question that is answered afresh.
        //
        // Succession then needs no rule at all, which is the whole argument. Nothing here
        // schedules an inheritance or records one — the head dies, somebody asks again, and
        // the answer is somebody else.
        let (_people, ids) = population(3);
        let mut society = Society::default();
        let home = society.found_household(Time::ORIGIN, 0.5);
        for id in &ids {
            society.move_in(*id, home);
        }

        // Whoever has most to put behind a claim, whatever their age.
        let standing = |id: PersonId, alive: &[PersonId]| {
            alive.contains(&id).then(|| {
                let rank = ids.iter().position(|other| *other == id).unwrap_or(0);
                // Descending, so the first is the strongest.
                (1.0 - rank as f32 * 0.25, 40.0 - rank as f64)
            })
        };

        let household = society.household(home).expect("a household");
        let alive = ids.clone();
        assert_eq!(
            household.head(|id| standing(id, &alive)),
            Some(ids[0]),
            "the strongest member speaks for it"
        );

        // The head dies. Nobody hands anything over and nothing is written down.
        let after = ids[1..].to_vec();
        assert_eq!(
            household.head(|id| standing(id, &after)),
            Some(ids[1]),
            "and when they are gone the next question returns somebody else"
        );

        // A household with no grown adult left in it speaks for nobody, rather than
        // speaking for a corpse.
        assert_eq!(household.head(|_| None), None, "an empty house has no head");
    }

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
