//! Who may add the next block, and how everybody else agrees to it.
//!
//! Byzantine fault tolerance in the Tendermint family, reduced to its load-bearing parts:
//!
//! - **A validator set**, weighted by stake. Power is whole coins bonded.
//! - **A proposer for every height and round**, chosen by weighted round-robin: every
//!   validator's priority rises by its power each round, the highest proposes, and the
//!   proposer's priority falls by the total. Over time each validator proposes in exact
//!   proportion to its stake, deterministically, with nobody able to grind their way to
//!   proposing more often.
//! - **A commit**: precommit votes, each an Ed25519 signature over (chain, height, round,
//!   block hash), from validators holding **more than two thirds** of the power.
//!
//! Why two thirds is the whole argument, briefly. Two commits for different blocks at one
//! height would each need more than two thirds of the power, so they would overlap in more
//! than a third — and every validator in the overlap signed both. That holds whatever rounds
//! the two commits were made in, and so the offence is two blocks at one height, not two votes
//! in one round: an honest validator signs one block a height (`Chain::step`), and
//! `tx::Action::Evidence` slashes whoever signs a second, in the same round or a later one. So
//! a fork cannot happen unless more than a third of the stake is willing to be destroyed, a
//! block once committed is final, and anybody shown both sides of a fork can name who made it
//! (`light::fork`). And the chain keeps moving as long as more than two thirds are answering: a
//! round whose proposer is absent, or whose block too few are there to sign, simply fails, and
//! the next round has a different proposer — who puts the same block forward again if anybody
//! signed it.

use crate::codec::{Malformed, Reader, Writer};
use crate::merkle;
use crate::tx::Address;
use crate::{Digest, PublicKey, Signature, SigningKey};

/// One validator's signature that a block should be the block at a height.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vote {
    pub chain: Digest,
    pub height: u64,
    pub round: u32,
    pub block: Digest,
    pub validator: PublicKey,
    pub signature: Signature,
}

impl Vote {
    pub fn signed(key: &SigningKey, chain: Digest, height: u64, round: u32, block: Digest) -> Vote {
        let mut vote = Vote {
            chain,
            height,
            round,
            block,
            validator: key.public(),
            signature: Signature([0; 64]),
        };
        vote.signature = key.sign(&vote.payload());
        vote
    }

    /// What is signed. Tagged, so no transaction can ever encode to a vote.
    pub fn payload(&self) -> Vec<u8> {
        Writer::tagged("life-rs/chain/precommit/1")
            .fixed(&self.chain.0)
            .u64(self.height)
            .u32(self.round)
            .fixed(&self.block.0)
            .finish()
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = self.payload();
        bytes.extend_from_slice(&self.validator.0);
        bytes.extend_from_slice(&self.signature.0);
        bytes
    }

    /// `encode`, backwards.
    pub fn decode(bytes: &[u8]) -> Result<Vote, Malformed> {
        let mut r = Reader::tagged(bytes, "life-rs/chain/precommit/1")?;
        let vote = Vote {
            chain: Digest(r.fixed()?),
            height: r.u64()?,
            round: r.u32()?,
            block: Digest(r.fixed()?),
            validator: PublicKey(r.fixed()?),
            signature: Signature(r.fixed()?),
        };
        r.done()?;
        Ok(vote)
    }

    pub fn signature_holds(&self) -> bool {
        self.validator.verify(&self.payload(), &self.signature)
    }
}

/// The votes that made a block final.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Commit {
    pub round: u32,
    pub votes: Vec<Vote>,
}

impl Commit {
    pub fn hash(&self) -> Digest {
        let leaves: Vec<Digest> = self.votes.iter().map(|v| merkle::leaf(&v.encode())).collect();
        let mut w = Writer::tagged("life-rs/chain/commit/1");
        w.u32(self.round).fixed(&merkle::root(&leaves).0);
        Digest::of(&w.finish())
    }
}

/// A validator: an address, the key it signs with, its power, and where it stands in the
/// rotation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Validator {
    pub address: Address,
    pub key: PublicKey,
    pub power: u64,
    pub priority: i64,
}

/// Why a commit does not make a block final.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoQuorum {
    /// A vote for some other chain, height, round or block.
    StrayVote,
    /// A vote from a key that is not in the set.
    Stranger,
    /// The same validator twice.
    Repeated,
    BadSignature,
    /// Signed, but by too little of the stake.
    TooLittlePower { signed: u64, total: u64 },
}

/// Everybody who may validate a height, in address order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidatorSet {
    pub members: Vec<Validator>,
}

impl ValidatorSet {
    /// The set that follows `previous`, given who is now eligible and with how much power.
    ///
    /// A validator who stays keeps their place in the rotation. One who arrives starts well
    /// behind — at minus one and an eighth of the total, as Tendermint has it — so that
    /// bonding a large stake does not buy the very next proposal.
    pub fn after(previous: &ValidatorSet, eligible: &[(Address, PublicKey, u64)]) -> ValidatorSet {
        let total: i64 = eligible.iter().map(|(_, _, p)| *p as i64).sum();
        let mut members: Vec<Validator> = eligible
            .iter()
            .filter(|(_, _, power)| *power > 0)
            .map(|(address, key, power)| {
                let priority = previous
                    .members
                    .iter()
                    .find(|v| v.address == *address)
                    .map(|v| v.priority)
                    .unwrap_or(-(total + total / 8));
                Validator {
                    address: *address,
                    key: *key,
                    power: *power,
                    priority,
                }
            })
            .collect();
        members.sort_by_key(|v| v.address);
        let mut set = ValidatorSet { members };
        set.recentre();
        set
    }

    pub fn total_power(&self) -> u64 {
        self.members.iter().map(|v| v.power).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn find(&self, key: &PublicKey) -> Option<usize> {
        self.members.iter().position(|v| v.key == *key)
    }

    /// What a header commits to: who, and with how much power — not where anybody stands in
    /// the rotation, which is derived.
    pub fn hash(&self) -> Digest {
        let leaves: Vec<Digest> = self
            .members
            .iter()
            .map(|v| {
                merkle::leaf(
                    &Writer::tagged("life-rs/chain/validator/1")
                        .fixed(&v.address.0)
                        .fixed(&v.key.0)
                        .u64(v.power)
                        .finish(),
                )
            })
            .collect();
        merkle::root(&leaves)
    }

    /// One turn of the rotation: everybody gains their power, the highest proposes and pays
    /// the total back. Ties go to the lowest address.
    fn turn(&mut self) -> usize {
        let total = self.total_power() as i64;
        for v in &mut self.members {
            v.priority += v.power as i64;
        }
        let mut best = 0;
        for (at, v) in self.members.iter().enumerate() {
            if v.priority > self.members[best].priority {
                best = at;
            }
        }
        self.members[best].priority -= total;
        best
    }

    /// Who proposes at a round, and the rotation as it stands once they have: round zero is
    /// one turn from where the last height left it, and every failed round is one more.
    pub fn at_round(&self, round: u32) -> (ValidatorSet, usize) {
        let mut set = self.clone();
        // An empty set has nobody to turn to; whoever asks must check before indexing.
        if set.members.is_empty() {
            return (set, 0);
        }
        let mut proposer = 0;
        for _ in 0..=round {
            proposer = set.turn();
        }
        set.recentre();
        (set, proposer)
    }

    /// Keep priorities centred on zero and within twice the total of each other, so that they
    /// stay bounded however long the chain runs.
    fn recentre(&mut self) {
        if self.members.is_empty() {
            return;
        }
        let total = self.total_power() as i64;
        let spread = self.members.iter().map(|v| v.priority).max().unwrap_or(0)
            - self.members.iter().map(|v| v.priority).min().unwrap_or(0);
        let limit = 2 * total.max(1);
        if spread > limit {
            let ratio = (spread + limit - 1) / limit;
            for v in &mut self.members {
                v.priority /= ratio;
            }
        }
        let mean = self.members.iter().map(|v| v.priority).sum::<i64>() / self.members.len() as i64;
        for v in &mut self.members {
            v.priority -= mean;
        }
    }

    /// Whether a commit makes `block` final at this height and round: every vote for exactly
    /// that, each from a distinct member, each signature good, and more than two thirds of the
    /// power among them. Returns the power that signed.
    pub fn check(
        &self,
        commit: &Commit,
        chain: Digest,
        height: u64,
        block: Digest,
    ) -> Result<u64, NoQuorum> {
        let mut seen = vec![false; self.members.len()];
        let mut signed = 0u64;
        for vote in &commit.votes {
            if vote.chain != chain
                || vote.height != height
                || vote.round != commit.round
                || vote.block != block
            {
                return Err(NoQuorum::StrayVote);
            }
            let Some(at) = self.find(&vote.validator) else {
                return Err(NoQuorum::Stranger);
            };
            if seen[at] {
                return Err(NoQuorum::Repeated);
            }
            seen[at] = true;
            if !vote.signature_holds() {
                return Err(NoQuorum::BadSignature);
            }
            signed += self.members[at].power;
        }
        let total = self.total_power();
        if 3 * signed as u128 > 2 * total as u128 {
            Ok(signed)
        } else {
            Err(NoQuorum::TooLittlePower { signed, total })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(powers: &[u64]) -> (ValidatorSet, Vec<SigningKey>) {
        let keys: Vec<SigningKey> = (0..powers.len())
            .map(|i| SigningKey::from_seed([i as u8 + 1; 32]))
            .collect();
        let eligible: Vec<(Address, PublicKey, u64)> = keys
            .iter()
            .zip(powers)
            .map(|(k, p)| (Address::of(&k.public()), k.public(), *p))
            .collect();
        (ValidatorSet::after(&ValidatorSet::default(), &eligible), keys)
    }

    /// Over many heights each validator proposes in proportion to its stake — exactly, not on
    /// average, because the rotation is a deterministic schedule and not a lottery.
    #[test]
    fn proposers_take_turns_in_proportion_to_stake() {
        let (mut rotation, _) = set(&[1, 2, 3, 4]);
        let mut proposed = [0u32; 4];
        for _ in 0..1000 {
            let (next, proposer) = rotation.at_round(0);
            proposed[proposer] += 1;
            rotation = next;
        }
        let powers: Vec<u64> = rotation.members.iter().map(|v| v.power).collect();
        for (count, power) in proposed.iter().zip(&powers) {
            let expected = 1000.0 * *power as f64 / 10.0;
            assert!(
                (*count as f64 - expected).abs() <= 2.0,
                "a validator with {power} of 10 proposed {count} of 1000"
            );
        }
    }

    #[test]
    fn a_failed_round_hands_the_proposal_to_somebody_else() {
        let (rotation, _) = set(&[5, 5, 5, 5]);
        let (_, first) = rotation.at_round(0);
        let (_, second) = rotation.at_round(1);
        assert_ne!(first, second);
    }

    #[test]
    fn more_than_two_thirds_is_a_quorum_and_two_thirds_is_not() {
        let (validators, keys) = set(&[1, 1, 1]);
        let chain = Digest::of(b"c");
        let block = Digest::of(b"b");
        let vote = |i: usize| {
            let key = keys
                .iter()
                .find(|k| k.public() == validators.members[i].key)
                .unwrap();
            Vote::signed(key, chain, 7, 0, block)
        };
        let two = Commit {
            round: 0,
            votes: vec![vote(0), vote(1)],
        };
        assert_eq!(
            validators.check(&two, chain, 7, block),
            Err(NoQuorum::TooLittlePower {
                signed: 2,
                total: 3
            }),
            "exactly two thirds is not more than two thirds"
        );
        let all = Commit {
            round: 0,
            votes: vec![vote(0), vote(1), vote(2)],
        };
        assert_eq!(validators.check(&all, chain, 7, block), Ok(3));
        let twice = Commit {
            round: 0,
            votes: vec![vote(0), vote(1), vote(1)],
        };
        assert_eq!(validators.check(&twice, chain, 7, block), Err(NoQuorum::Repeated));
        assert_eq!(
            validators.check(&all, chain, 7, Digest::of(b"another")),
            Err(NoQuorum::StrayVote)
        );
        let outsider = SigningKey::from_seed([99; 32]);
        let strange = Commit {
            round: 0,
            votes: vec![vote(0), vote(1), Vote::signed(&outsider, chain, 7, 0, block)],
        };
        assert_eq!(validators.check(&strange, chain, 7, block), Err(NoQuorum::Stranger));
    }

    #[test]
    fn priorities_stay_bounded() {
        let (mut rotation, _) = set(&[1, 1000, 7, 3]);
        for _ in 0..10_000 {
            rotation = rotation.at_round(0).0;
        }
        let total = rotation.total_power() as i64;
        for v in &rotation.members {
            assert!(v.priority.abs() <= 3 * total, "priority {} ran away", v.priority);
        }
    }
}
