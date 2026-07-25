//! Generational handles and the arenas they index into.
//!
//! Entities are named by `Id<T>`, never by reference. This is what lets a family be
//! a cycle, a food web have loops, and any entity be addressed at any time without
//! threading lifetimes through the whole simulation.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// A handle to an entity of type `T` living in an [`Arena`].
///
/// The generation counter is what makes death safe: when a slot is reused, handles
/// to the previous occupant stop resolving instead of silently pointing at a
/// stranger.
pub struct Id<T> {
    index: u32,
    generation: u32,
    // `fn() -> T` rather than `T` so `Id<T>` is Copy/Send/Sync whatever `T` is.
    _marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    fn new(index: u32, generation: u32) -> Self {
        Id {
            index,
            generation,
            _marker: PhantomData,
        }
    }

    pub fn index(self) -> u32 {
        self.index
    }

    pub fn generation(self) -> u32 {
        self.generation
    }

    /// Stable scalar form, for seeding RNG streams and for save files.
    pub fn to_bits(self) -> u64 {
        (u64::from(self.generation) << 32) | u64::from(self.index)
    }
}

// Derived impls would demand `T: Clone`, `T: Hash`, and so on. A handle's identity
// has nothing to do with the pointee's capabilities, so they are written out.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Id<T> {}
impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.generation == other.generation
    }
}
impl<T> Eq for Id<T> {}
impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_bits().hash(state);
    }
}
impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.to_bits().cmp(&other.to_bits())
    }
}
impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}v{}", self.index, self.generation)
    }
}

struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

/// A generational arena: stable handles, O(1) access, slot reuse after removal.
///
/// Iteration is in slot order, which is stable for a given sequence of
/// inserts and removes — systems that iterate must not introduce nondeterminism.
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    len: usize,
}

impl<T> Arena<T> {
    pub fn new() -> Self {
        Arena {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Arena {
            slots: Vec::with_capacity(capacity),
            free: Vec::new(),
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn insert(&mut self, value: T) -> Id<T> {
        self.len += 1;
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            debug_assert!(slot.value.is_none(), "free list pointed at a live slot");
            slot.value = Some(value);
            Id::new(index, slot.generation)
        } else {
            let index = u32::try_from(self.slots.len()).expect("arena exceeded 2^32 slots");
            self.slots.push(Slot {
                generation: 0,
                value: Some(value),
            });
            Id::new(index, 0)
        }
    }

    pub fn get(&self, id: Id<T>) -> Option<&T> {
        let slot = self.slots.get(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        slot.value.as_ref()
    }

    pub fn get_mut(&mut self, id: Id<T>) -> Option<&mut T> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        slot.value.as_mut()
    }

    pub fn contains(&self, id: Id<T>) -> bool {
        self.get(id).is_some()
    }

    pub fn remove(&mut self, id: Id<T>) -> Option<T> {
        let slot = self.slots.get_mut(id.index as usize)?;
        if slot.generation != id.generation {
            return None;
        }
        let value = slot.value.take()?;
        // Bumping on removal is what invalidates outstanding handles.
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(id.index);
        self.len -= 1;
        Some(value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (Id<T>, &T)> {
        self.slots.iter().enumerate().filter_map(|(i, slot)| {
            let value = slot.value.as_ref()?;
            Some((Id::new(i as u32, slot.generation), value))
        })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<T>, &mut T)> {
        self.slots.iter_mut().enumerate().filter_map(|(i, slot)| {
            let generation = slot.generation;
            let value = slot.value.as_mut()?;
            Some((Id::new(i as u32, generation), value))
        })
    }

    pub fn ids(&self) -> impl Iterator<Item = Id<T>> + '_ {
        self.iter().map(|(id, _)| id)
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Thing(u32);

    #[test]
    fn insert_and_get() {
        let mut arena = Arena::new();
        let a = arena.insert(Thing(1));
        let b = arena.insert(Thing(2));
        assert_eq!(arena.get(a), Some(&Thing(1)));
        assert_eq!(arena.get(b), Some(&Thing(2)));
        assert_eq!(arena.len(), 2);
    }

    #[test]
    fn removal_invalidates_the_handle() {
        let mut arena = Arena::new();
        let a = arena.insert(Thing(1));
        assert_eq!(arena.remove(a), Some(Thing(1)));
        assert_eq!(arena.get(a), None);
        assert_eq!(arena.remove(a), None);
        assert!(arena.is_empty());
    }

    #[test]
    fn a_reused_slot_does_not_answer_to_the_old_handle() {
        let mut arena = Arena::new();
        let dead = arena.insert(Thing(1));
        arena.remove(dead);
        let live = arena.insert(Thing(2));

        // Same slot, different generation: the point of the whole exercise.
        assert_eq!(dead.index(), live.index());
        assert_ne!(dead, live);
        assert_eq!(arena.get(dead), None);
        assert_eq!(arena.get(live), Some(&Thing(2)));
    }

    #[test]
    fn iteration_skips_holes_and_stays_ordered() {
        let mut arena = Arena::new();
        let a = arena.insert(Thing(1));
        let b = arena.insert(Thing(2));
        let c = arena.insert(Thing(3));
        arena.remove(b);

        let seen: Vec<_> = arena.iter().map(|(id, t)| (id, t.0)).collect();
        assert_eq!(seen, vec![(a, 1), (c, 3)]);
    }

    #[test]
    fn handles_are_ordered_and_hashable() {
        let mut arena = Arena::new();
        let a = arena.insert(Thing(1));
        let b = arena.insert(Thing(2));
        assert!(a < b);

        let set: std::collections::HashSet<_> = [a, b, a].into_iter().collect();
        assert_eq!(set.len(), 2);
    }
}
