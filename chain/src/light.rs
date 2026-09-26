//! Following a chain without keeping it.
//!
//! Replaying from genesis (`Chain::replay`) is the strongest check there is and the dearest: it
//! applies every transaction again. Most parties to a ledger want something cheaper — to know
//! that a header is one the chain's validators really made final, so that a proof against its
//! state root can be believed. That is what a light client does, and this is the sequential
//! kind Tendermint's clients run.
//!
//! It holds the genesis and nothing else. For each height it is handed a *light block*: the
//! header, the commit that made it final, and the validator set the header names. It checks that
//! the header follows the one before — height, parent, time, the commit before it — that the set
//! is the one the previous header said would sign next and hashes to what this header names, and
//! that more than two thirds of that set's power signed this header. It never sees a transaction
//! or an account. What it cannot check itself, that the transactions inside were valid, it takes
//! from the two thirds: to sign a bad block a forger needs that much of the stake, and signing
//! two blocks at one height is evidence that costs a validator its stake.
//!
//! And if two histories both check, the light client has caught a fork, and `fork` says who made
//! it. Both were signed by one set where they part, each block by more than two thirds of its
//! power, so more than a third signed both — and every one of them is named, with the two votes
//! that show it, as the chain takes them.

use crate::block::Header;
use crate::consensus::{Commit, ValidatorSet, Vote};
use crate::node::{Genesis, Invalid, genesis_header_of, made_final};
use crate::Digest;

/// What a light client is handed for one height.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LightBlock {
    pub header: Header,
    pub commit: Commit,
    /// The set that signed it — the one the header names by hash.
    pub validators: ValidatorSet,
}

/// Follow a chain from its genesis through light blocks for every height from zero. Returns the
/// height of the last header it can trust, or the first height that fails and why.
pub fn follow(genesis: &Genesis, blocks: &[LightBlock]) -> Result<u64, (u64, Invalid)> {
    genesis.check().map_err(|why| (0, Invalid::BadGenesis(why)))?;
    let Some(first) = blocks.first() else {
        return Err((0, Invalid::NotGenesis));
    };
    // The first header is the one anybody holding the genesis can build for themselves.
    if first.header != genesis_header_of(genesis) {
        return Err((0, Invalid::NotGenesis));
    }
    follow_from(first, &blocks[1..])
}

/// Follow a chain from a header already trusted — a checkpoint somebody was given, or one they
/// followed to before — through light blocks for every height after it, as most light clients
/// start. The trusted block's own commit is checked too; what cannot be checked from here is
/// that the checkpoint is really the chain's, which is what trusting it means. Returns the height
/// of the last header it can trust, or the first that fails and why.
pub fn follow_from(trusted: &LightBlock, blocks: &[LightBlock]) -> Result<u64, (u64, Invalid)> {
    let id = trusted.header.chain;
    is_final(trusted, id).map_err(|why| (trusted.header.height, why))?;
    let mut before = trusted;
    for next in blocks {
        follows(before, next, id).map_err(|why| (next.header.height, why))?;
        before = next;
    }
    Ok(before.header.height)
}

/// Whether a light block's commit makes its header final by the set it carries, and that set is
/// the one its header names.
fn is_final(block: &LightBlock, id: Digest) -> Result<(), Invalid> {
    if block.header.chain != id {
        return Err(Invalid::WrongChain);
    }
    if block.validators.hash() != block.header.validators {
        return Err(Invalid::WrongValidators);
    }
    made_final(&block.validators, &block.commit, &block.header)?;
    Ok(())
}

/// Whether `next` comes after `before`, is signed by the set `before` handed over to, and is
/// final.
fn follows(before: &LightBlock, next: &LightBlock, id: Digest) -> Result<(), Invalid> {
    let (was, now) = (&before.header, &next.header);
    if now.height != was.height + 1 {
        return Err(Invalid::NotNext {
            expected: was.height + 1,
            got: now.height,
        });
    }
    if now.parent != was.hash() {
        return Err(Invalid::WrongParent);
    }
    if now.last_commit != before.commit.hash() {
        return Err(Invalid::WrongLastCommit);
    }
    if now.time <= was.time {
        return Err(Invalid::TimeGoesBackwards);
    }
    if now.validators != was.next_validators {
        return Err(Invalid::WrongValidators);
    }
    is_final(next, id)
}

/// Where two histories part, and who parted them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fork {
    /// The first height at which they hold different blocks.
    pub height: u64,
    /// Everybody who signed both blocks there, each with a vote for either — which is what
    /// `Action::Evidence` takes.
    pub culprits: Vec<(Vote, Vote)>,
    /// Their power, out of the set's.
    pub power: u64,
    pub total: u64,
}

/// Whether two histories that follow from one trusted header are one chain or two.
///
/// Each is followed on its own first, and one that does not follow is refused as `follow_from`
/// refuses it: a forgery is nobody's fork, since nobody's signature makes it final. If both
/// follow, they agree up to some height and hold different blocks there — both final, both
/// signed by the one set the header before handed over to — and everybody in both commits is
/// named. Returns `None` if one history is the other with less of it.
pub fn fork(
    trusted: &LightBlock,
    one: &[LightBlock],
    other: &[LightBlock],
) -> Result<Option<Fork>, (u64, Invalid)> {
    follow_from(trusted, one)?;
    follow_from(trusted, other)?;
    let Some((a, b)) = one.iter().zip(other).find(|(a, b)| a.header != b.header) else {
        return Ok(None);
    };
    let set = &a.validators;
    let mut culprits = Vec::new();
    let mut power = 0;
    for first in &a.commit.votes {
        let Some(second) = b.commit.votes.iter().find(|v| v.validator == first.validator) else {
            continue;
        };
        if let Some(at) = set.find(&first.validator) {
            power += set.members[at].power;
            culprits.push((first.clone(), second.clone()));
        }
    }
    Ok(Some(Fork {
        height: a.header.height,
        culprits,
        power,
        total: set.total_power(),
    }))
}
