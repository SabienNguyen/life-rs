//! Ways of doing things, and the peoples who share them.
//!
//! The eight countries this project used to have were `Usa`, `Gbr`, `Deu`, `Can`, `Fra`,
//! `Chn`, `Jpn` and `Vnm` — an enum, inherited from your mother, attached to nothing. On a
//! planet drawn from a random seed, orbiting a star of nine tenths a solar mass, a woman
//! living on savanna at thirty degrees north would introduce herself as being from the
//! United States. It is the plainest violation of design principle one in the codebase:
//! **nothing is placed by fiat**, and there they were, eight of them, placed by fiat.
//!
//! So nobody writes down a country here. What is written down is the mechanism by which
//! people come to have one, and the countries are what the mechanism produces.
//!
//! ## Culture is norms with a memory
//!
//! §14 already gives every place a vector of `norms` — how prevalent each way of spending a
//! day is, read off what people there actually did. It is recomputed from scratch every
//! reckoning, so it is a *statistic* rather than a culture: it has no inertia, no
//! transmission, and no identity, and two places with the same numbers are not thereby
//! related.
//!
//! Culture is the same vector with three things added, and they are exactly the three that
//! make genetics genetics:
//!
//! **Transmission.** A place's ways move towards its neighbours' in proportion to how much
//! contact there is. Contact is reach — the roads that carry grain carry manners.
//!
//! **Drift.** Ways wander at random, and *faster in small populations*. This is the
//! cultural analogue of genetic drift and it has the same cause: fewer carriers, fewer
//! copies, more sampling error per generation. It is why isolated peoples diverge even when
//! nothing about their circumstances differs.
//!
//! **Descent.** When a place has drifted far enough from the culture it belongs to, it
//! *is* a different culture — one with a name, a date, and a parent. That is the same
//! allopatric rule `evolution` uses on species, applied to ideas instead of genes, and it
//! gives cultures a phylogeny for the same reason.
//!
//! ## A country is a culture that touches itself
//!
//! No separate machinery. A country is a maximal set of places that share a culture *and*
//! can reach one another — which is what a country is: people who do things the same way,
//! close enough together to keep doing them the same way. Open a sea between two halves of
//! one and they drift apart and become two; leave them connected and they stay one however
//! far apart they are.
//!
//! It is named after its largest place, because that is how the overwhelming majority of
//! real countries got their names.
//!
//! ## What is not here
//!
//! Language, religion, kinship rules, law, and any notion of a state — no government, no
//! taxation, no army, no border a person could be stopped at. A country here is an extent
//! of shared practice, which is the older and broader meaning of the word and the one that
//! does not require inventing an institution. Conquest, in particular, is absent: countries
//! here merge by *converging* rather than by one taking another.

use person::Deed;
use sim_core::Rng;

pub mod naming;

/// How many ways of doing things a culture tracks.
///
/// The same vector §14 already keeps as `norms`, so a culture is not a new kind of thing
/// bolted on beside the neighbourhood model — it is the neighbourhood model's own numbers,
/// given inertia and a name.
pub const WAYS: usize = Deed::COUNT;

/// How strongly a place's ways are pulled towards its neighbours', per reckoning, at full
/// reach.
///
/// Slow. Manners move at the speed of people, and before railways that is a few days' walk
/// a generation. Fast transmission would make a continent one culture, which is the failure
/// mode to avoid: the interesting thing about culture is that it varies.
///
/// Quoted at *full exposure* — the rate for somebody who meets essentially nobody from
/// their own place, which is the position of a hamlet at the gates of a city. Immersion
/// like that assimilates people inside a generation, hence a seven-year half-life, and it
/// is much faster than `ADOPTION`: a person surrounded by strangers is being taught, not
/// drifting.
///
/// A place's actual rate is this times its exposure, and for an ordinary settlement among
/// settlements of its own size that is under a tenth of the quoted figure — well below
/// `ADOPTION`. That ordering is the one that matters and it falls out rather than being
/// set: what the land asks of you every day moves you more than neighbours a week away,
/// unless you are so outnumbered that the neighbours are most of who you meet.
///
/// It was 0.055 flat, with no exposure term at all, and under that a city chased a hamlet's
/// manners exactly as hard as the hamlet chased the city's.
const CONTAGION: f32 = 0.12;

/// The population a drift rate is quoted for.
///
/// Drift needs a reference size or the number means nothing. A hundred is a village: small
/// enough that one family's habit is a visible fraction of what everybody does, large enough
/// to be a real place rather than a household.
const A_VILLAGE: f32 = 100.0;

/// How far a contested way wanders in a village of `A_VILLAGE`, per reckoning.
///
/// Scaled by the square root of the population, which is where sampling error goes: a
/// hamlet of twenty moves at twice this and a town of ten thousand at a tenth of it, and
/// that asymmetry is most of why small isolated peoples are the distinctive ones.
///
/// Also scaled by how contested the way is — see `step`. The quoted figure is for a
/// practice half the village follows, which is where drift is fastest.
const DRIFT: f32 = 0.045;

/// How much of what a place did this year its ways take on.
///
/// Two percent — a half-life of about thirty-five years, so a way of doing things takes a
/// generation and a half to change and the change outlives the people who started it. This
/// is the whole difference between a culture and a statistic. §14's `norms` are recomputed
/// from scratch every reckoning, which makes them a photograph of this year; the same
/// numbers at two percent a year are what a place *is*, and a decade of unusual behaviour
/// leaves a mark on it rather than replacing it.
///
/// It was 0.20 when this crate was first written, and that was wrong in a way worth
/// recording: a three-year half-life means a place's ways are just last decade's behaviour
/// with extra steps, and drift can never accumulate against it. Six hundred years of total
/// isolation moved a hamlet 0.03 — the restoring force ate every step. Culture only exists
/// at all if it changes more slowly than the people practising it.
const ADOPTION: f32 = 0.02;

/// How far a place's ways must be from its culture's before it is a culture of its own.
///
/// Euclidean distance over the whole vector. Generous, because every place drifts a little
/// and a threshold that catches ordinary variation would make a new culture per village
/// per century — which is the same fountain problem `evolution` hit with speciation, and
/// the same answer.
const A_DIFFERENT_PEOPLE: f32 = 0.45;

/// How close two cultures must come before they are one again.
///
/// Well inside the divergence threshold, so a place near the line does not split and merge
/// on alternate reckonings. The same hysteresis the settlement thresholds use, for the same
/// reason.
const THE_SAME_PEOPLE: f32 = 0.20;

/// The fewest people who can be a people.
///
/// Dunbar's number, and used for the reason he arrived at it: a hundred and fifty is about
/// the largest group that holds together by everybody knowing everybody. Below it a place is
/// a band — its practices are the habits of a few families and they are volatile in exactly
/// the way `DRIFT` says, since sampling error at twenty carriers is enormous. Above it a
/// practice has to survive being passed to people you do not personally know, which is what
/// makes it a custom rather than a habit.
///
/// Without a floor the world names a people per hamlet. A run of a hundred souls across five
/// neighbourhoods produced three peoples inside three years, every one of them a
/// twenty-person quarter whose ways had rattled apart under drift. They still rattle — this
/// does not slow them down — but a band that drifts is a band that drifts, not a nation.
const ENOUGH_TO_BE_A_PEOPLE: u32 = 150;

/// The least contact a place needs before it can rejoin a people at all.
///
/// Barely above nothing, because the claim being made is weak: not that a place is well
/// connected, only that somebody occasionally arrives. A place at exactly zero reach is
/// sealed, and a sealed place goes its own way permanently however much it comes to
/// resemble the world outside.
const IN_TOUCH: f32 = 0.02;

/// A way of doing things, with a name and a history.
#[derive(Clone, Debug)]
pub struct Culture {
    pub name: String,
    /// What its people do **now**, averaged over its places and weighted by how many live
    /// in each.
    ///
    /// Live rather than a snapshot of the day it got its name, and that is load-bearing. A
    /// frozen record turns every people into a fountain: an isolated hamlet drifts past the
    /// threshold, takes a name, keeps drifting, and passes the threshold again a few
    /// decades later measured against a record nobody has practised since. Fifteen peoples
    /// out of one village. Measured against what its members currently do, a place alone in
    /// its culture is by definition *at* its culture, and it cannot leave itself.
    pub ways: [f32; WAYS],
    /// The culture it diverged from, if it is not an original.
    pub parent: Option<usize>,
    /// When it became distinct, in years from the founding.
    pub arose: u64,
    /// The place it arose in.
    pub hearth: usize,
    /// How many people practise it now. Zero means it is gone, though it stays in the list
    /// so that the peoples descended from it still know where they came from.
    pub souls: u32,
}

impl Culture {
    /// Whether anybody still does things this way.
    pub fn living(&self) -> bool {
        self.souls > 0
    }
}

/// A country: places that share a culture and can reach each other.
#[derive(Clone, Debug, PartialEq)]
pub struct Country {
    pub name: String,
    /// Which culture its people share.
    pub culture: usize,
    /// The places in it, in order.
    pub places: Vec<usize>,
}

/// Every culture in a world, and which place carries what.
pub struct Cultures {
    cultures: Vec<Culture>,
    /// The culture each place belongs to.
    belongs: Vec<usize>,
    /// The ways each place actually practises, which drift away from its culture's.
    practised: Vec<[f32; WAYS]>,
    /// Cultures that have arisen, ever, including any since merged away.
    pub ever: usize,
    /// Times a place has rejoined a people it had drifted out of.
    pub merged: usize,
    /// Times a place has gone its own way again, back into a people it had been before.
    pub revivals: usize,
}

impl Cultures {
    /// One people, in every place, to begin with.
    ///
    /// A world starts culturally uniform and diverges, rather than starting with a set of
    /// peoples somebody chose. Everything that distinguishes them afterwards is drift and
    /// distance.
    pub fn beginning(places: usize, hearth_name: impl Into<String>) -> Cultures {
        let ways = [0.5; WAYS];
        Cultures {
            cultures: vec![Culture {
                name: hearth_name.into(),
                ways,
                parent: None,
                arose: 0,
                hearth: 0,
                souls: 0,
            }],
            belongs: vec![0; places],
            practised: vec![ways; places],
            ever: 1,
            merged: 0,
            revivals: 0,
        }
    }

    /// How many peoples there are, living or not.
    pub fn len(&self) -> usize {
        self.cultures.len()
    }

    /// How many places this world's culture knows about.
    pub fn places(&self) -> usize {
        self.belongs.len()
    }

    /// Every people, in the order they arose.
    pub fn all(&self) -> &[Culture] {
        &self.cultures
    }

    pub fn is_empty(&self) -> bool {
        self.cultures.is_empty()
    }

    pub fn get(&self, culture: usize) -> Option<&Culture> {
        self.cultures.get(culture)
    }

    /// Which culture a place belongs to.
    pub fn of_place(&self, place: usize) -> usize {
        self.belongs.get(place).copied().unwrap_or(0)
    }

    /// What a place actually does, as against what its culture nominally is.
    pub fn practised(&self, place: usize) -> [f32; WAYS] {
        self.practised
            .get(place)
            .copied()
            .unwrap_or([0.5; WAYS])
    }

    /// What this culture descends from, nearest first.
    pub fn ancestry(&self, culture: usize) -> Vec<usize> {
        let mut chain = Vec::new();
        let mut at = self.cultures.get(culture).and_then(|c| c.parent);
        while let Some(parent) = at {
            if chain.contains(&parent) {
                break;
            }
            chain.push(parent);
            at = self.cultures.get(parent).and_then(|c| c.parent);
        }
        chain
    }

    /// Make room for places founded since the last reckoning.
    ///
    /// A new place starts practising whatever its nearest inhabited neighbour does, because
    /// somebody walked there from somewhere.
    pub fn extend_to(&mut self, places: usize, from: Option<usize>) {
        while self.belongs.len() < places {
            let (culture, ways) = match from.filter(|f| *f < self.belongs.len()) {
                Some(f) => (self.belongs[f], self.practised[f]),
                None => (0, [0.5; WAYS]),
            };
            self.belongs.push(culture);
            self.practised.push(ways);
        }
    }

    /// A reckoning's worth of culture: what people did, who they met, and chance.
    ///
    /// `doing` is what each place's residents actually did this year — the norms §14 already
    /// computes. `contact` is how reachable each place is, and `souls` how many live there.
    /// Places with nobody in them are left exactly as they were: an emptied village does not
    /// forget its manners, it simply has nobody practising them.
    pub fn step(
        &mut self,
        doing: &[[f32; WAYS]],
        contact: &[f32],
        souls: &[u32],
        year: u64,
        rng: &mut Rng,
    ) {
        let places = self.belongs.len().min(doing.len()).min(contact.len()).min(souls.len());
        if places == 0 {
            return;
        }

        // What everybody is doing on average, weighted by how reachable they are. This
        // stands in for "the neighbours a place actually has": at these scales every
        // settlement in a region is a few days from every other, and which particular one
        // you borrow a habit from matters far less than that you borrow it from somewhere.
        let mut heard = [0.0f32; WAYS];
        let mut weight = 0.0;
        for place in 0..places {
            if souls[place] == 0 {
                continue;
            }
            let w = contact[place].clamp(0.0, 1.0) * souls[place] as f32;
            weight += w;
            for way in 0..WAYS {
                heard[way] += w * self.practised[place][way];
            }
        }
        if weight > 0.0 {
            for way in &mut heard {
                *way /= weight;
            }
        }

        for place in 0..places {
            if souls[place] == 0 {
                continue;
            }
            let reach = contact[place].clamp(0.0, 1.0);
            // How much of the company a person here keeps is from somewhere else. A scalar
            // reach is not enough on its own: a hamlet of thirty at the gates of a city of
            // nine hundred and the city itself have exactly the same roads, but almost
            // everybody the hamlet meets is a stranger and almost nobody the city meets is.
            // Weighting the pull by that share is what stops a city from chasing a hamlet's
            // manners, and what stops a hamlet in daily contact with a nation from drifting
            // freely as though it were alone.
            let abroad = (weight - reach * souls[place] as f32).max(0.0);
            let exposure = reach * abroad / (abroad + souls[place] as f32).max(1.0);
            // Drift falls as the square root of the number of carriers, which is where
            // sampling error goes. A hamlet reinvents itself; a city barely moves.
            let wobble = DRIFT * (A_VILLAGE / (souls[place] as f32).max(1.0)).sqrt();

            for way in 0..WAYS {
                let mine = self.practised[place][way];
                // What people here actually did, taken on slowly — a culture is a practice
                // before it is an inheritance, but it is a practice with a memory.
                let towards_practice = (doing[place][way] - mine) * ADOPTION;
                // What they hear from elsewhere, in proportion to how much they hear.
                let towards_others = (heard[way] - mine) * CONTAGION * exposure;
                // Sampling error is largest when a practice is genuinely contested and
                // vanishes when nearly everybody or nearly nobody does it — the same
                // `p(1-p)/N` the genetics in this project already runs on, because it is the
                // same phenomenon. A way that ninety-nine people in a hundred follow is what
                // ninety-nine children in a hundred see, and it does not wander.
                let contested = (mine * (1.0 - mine) / 0.25).max(0.0).sqrt();
                let chance = (rng.unit_f32() - 0.5) * 2.0 * wobble * contested;
                self.practised[place][way] =
                    (mine + towards_practice + towards_others + chance).clamp(0.0, 1.0);
            }
        }

        self.reckon(souls);
        self.diverge(souls, year, rng);
        self.reckon(souls);
        self.converge(places, contact);
        self.reckon(souls);
    }

    /// Read every culture off the places that practise it.
    ///
    /// A people is its people. Nothing here decides what a culture is; it is the average of
    /// what its members do, weighted by how many of them there are, so a culture with one
    /// large town and four hamlets is mostly the town — which is both true and the reason
    /// hamlets are the ones that leave.
    fn reckon(&mut self, souls: &[u32]) {
        let places = self.belongs.len().min(souls.len());
        let mut total = vec![[0.0f64; WAYS]; self.cultures.len()];
        let mut weight = vec![0.0f64; self.cultures.len()];

        for place in 0..places {
            let living = souls[place];
            if living == 0 {
                continue;
            }
            let culture = self.belongs[place];
            if culture >= self.cultures.len() {
                continue;
            }
            weight[culture] += living as f64;
            for way in 0..WAYS {
                total[culture][way] += living as f64 * self.practised[place][way] as f64;
            }
        }

        for (culture, people) in self.cultures.iter_mut().enumerate() {
            people.souls = weight[culture] as u32;
            if weight[culture] <= 0.0 {
                // Nobody left. The ways stay as they last were, so the record of a people
                // that died out is what they were doing when they did.
                continue;
            }
            for way in 0..WAYS {
                people.ways[way] = (total[culture][way] / weight[culture]) as f32;
            }
        }
    }

    /// What a people do, leaving out one of their places.
    ///
    /// `None` when that place is the only one practising it — a people of one place has no
    /// rest of itself, and is therefore never at any distance from it.
    fn rest_of(&self, culture: usize, without: usize, souls: &[u32]) -> Option<[f32; WAYS]> {
        let places = self.belongs.len().min(souls.len());
        let mut total = [0.0f64; WAYS];
        let mut weight = 0.0f64;
        for place in 0..places {
            if place == without || souls[place] == 0 || self.belongs[place] != culture {
                continue;
            }
            weight += souls[place] as f64;
            for way in 0..WAYS {
                total[way] += souls[place] as f64 * self.practised[place][way] as f64;
            }
        }
        if weight <= 0.0 {
            return None;
        }
        let mut theirs = [0.0f32; WAYS];
        for way in 0..WAYS {
            theirs[way] = (total[way] / weight) as f32;
        }
        Some(theirs)
    }

    /// Places that have drifted far enough become peoples of their own.
    fn diverge(&mut self, souls: &[u32], year: u64, rng: &mut Rng) {
        for place in 0..self.belongs.len().min(souls.len()) {
            if souls[place] < ENOUGH_TO_BE_A_PEOPLE {
                continue;
            }
            let mine = self.practised[place];
            let culture = self.belongs[place];
            // Measured against *the rest of* this people, not against a mean the place is
            // itself inside. Including yourself in your own reference point halves every
            // distance you could ever be from it: two equal halves of one culture put its
            // ways exactly between them, so each sits at half their true separation and
            // neither can ever leave, however differently they live. Excluding yourself
            // also makes the threshold mean the same thing whether a culture has two places
            // or twenty.
            let Some(theirs) = self.rest_of(culture, place, souls) else {
                // Nobody else practises it, so there is nothing to be different from. This
                // is what stops a place alone in its own culture from endlessly re-leaving
                // itself.
                continue;
            };
            if distance(&mine, &theirs) < A_DIFFERENT_PEOPLE {
                continue;
            }
            // The smaller part is the part that leaves. Distance is mutual — if a hamlet is
            // far from its nation then its nation is exactly as far from the hamlet — so
            // without this the first place scanned wins, and a city of nine hundred
            // announces that it has broken away from a village of thirty. A people is where
            // most of it lives; you leave it, it does not leave you.
            let whole = self.cultures[culture].souls;
            if souls[place] as u64 * 2 > whole as u64 {
                continue;
            }

            // A place that has gone its own way before, come back, and gone the same way
            // again is not a second people. It is the same people, returning to itself, and
            // it keeps the name it had — so the criterion is the one used everywhere else
            // for sameness: this place, leaving the same culture, arriving somewhere it has
            // already been.
            //
            // Without this a hamlet small enough to be a genuine random walk mints a name
            // every time it crosses the line, which over two millennia is eight peoples for
            // one village of twenty-five. The hysteresis gap stops the split-merge churn
            // *within* a reckoning; this stops it across centuries.
            let returning = self.cultures.iter().position(|old| {
                old.hearth == place
                    && !old.living()
                    && old.parent == Some(culture)
                    && distance(&mine, &old.ways) < THE_SAME_PEOPLE
            });
            if let Some(again) = returning {
                self.belongs[place] = again;
                self.revivals += 1;
                continue;
            }

            let name = naming::name_a_people(&mine, rng);
            self.cultures.push(Culture {
                name,
                ways: mine,
                parent: Some(culture),
                arose: year,
                hearth: place,
                souls: souls[place],
            });
            self.belongs[place] = self.cultures.len() - 1;
            self.ever += 1;
        }
    }

    /// Places doing very nearly the same thing, *and in touch with each other*, are one
    /// people again.
    ///
    /// Merging towards the *older* culture, so a people that split and came back is the
    /// people it was rather than a third thing. Without that, drifting back and forth
    /// produces an endless list of names for the same practice.
    ///
    /// Contact is the other requirement and it is the load-bearing one. A place nobody can
    /// reach cannot rejoin anybody, however much it happens to resemble them: resemblance
    /// across a gap nobody crosses is convergence, not kinship, and the same distinction
    /// `evolution` draws between two populations that look alike and two that interbreed.
    /// Merging on resemblance alone was what made an isolated hamlet churn — it would drift
    /// out, take a name, drift back into coincidental likeness with a mainland it had never
    /// once touched, and be absorbed by it.
    fn converge(&mut self, places: usize, contact: &[f32]) {
        for place in 0..places {
            if contact.get(place).copied().unwrap_or(0.0) <= IN_TOUCH {
                continue;
            }
            let mine = self.practised[place];
            let culture = self.belongs[place];
            let mut best: Option<(usize, f32)> = None;
            for (other, candidate) in self.cultures.iter().enumerate() {
                // Not itself, and not a people nobody practises any more — a place near the
                // recorded ways of an extinct culture has not rejoined it, it has merely
                // arrived somewhere they used to be.
                if other == culture || !candidate.living() {
                    continue;
                }
                let apart = distance(&mine, &candidate.ways);
                if apart < THE_SAME_PEOPLE && best.is_none_or(|(_, b)| apart < b) {
                    best = Some((other, apart));
                }
            }
            if let Some((other, _)) = best {
                // Only towards something older, which is what makes this a return rather
                // than a churn.
                let older = self.cultures[other].arose <= self.cultures[culture].arose;
                if older && other != culture {
                    self.belongs[place] = other;
                    self.merged += 1;
                }
            }
        }
    }

    /// The countries of this world: places sharing a culture that can reach each other.
    ///
    /// Derived every time rather than stored, because it is a *reading* of who does what
    /// where — the same rule §14 applies to archetypes. A country cannot fall out of step
    /// with its places if it is recomputed from them.
    pub fn countries(&self, souls: &[u32], reachable: impl Fn(usize, usize) -> bool) -> Vec<Country> {
        let places = self.belongs.len().min(souls.len());
        let mut seen = vec![false; places];
        let mut found = Vec::new();

        for start in 0..places {
            if seen[start] || souls[start] == 0 {
                continue;
            }
            let culture = self.belongs[start];
            let mut members = vec![start];
            seen[start] = true;
            // Everybody of the same culture this one can get to. Transitive, so a chain of
            // reachable places is one country even if its ends are far apart.
            let mut frontier = vec![start];
            while let Some(here) = frontier.pop() {
                for other in 0..places {
                    if seen[other] || souls[other] == 0 || self.belongs[other] != culture {
                        continue;
                    }
                    if reachable(here, other) {
                        seen[other] = true;
                        members.push(other);
                        frontier.push(other);
                    }
                }
            }
            members.sort_unstable();
            found.push(Country {
                // Named after its largest place, which is how most real countries got
                // their names. The caller supplies place names; this is the index of the
                // biggest, and `name_country` turns it into a word.
                name: String::new(),
                culture,
                places: members,
            });
        }

        // Largest first, so the reading is stable and the big ones are easy to find.
        found.sort_by_key(|c| std::cmp::Reverse(c.places.len()));
        found
    }
}

/// How far apart two ways of doing things are.
fn distance(a: &[f32; WAYS], b: &[f32; WAYS]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

#[cfg(test)]
mod tests;
