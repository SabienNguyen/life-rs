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

use crate::block::Header;
use crate::consensus::{Commit, ValidatorSet};
use crate::node::{Genesis, Invalid, genesis_header_of};
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
    block
        .validators
        .check(&block.commit, id, block.header.height, block.header.hash())
        .map_err(Invalid::Commit)?;
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
