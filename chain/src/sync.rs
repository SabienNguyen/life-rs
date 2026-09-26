//! Joining a chain late.
//!
//! `Chain::replay` checks everything from the genesis, and costs more with every block. A node
//! joining a chain that has run for centuries does what real ones do. It follows the headers
//! from the genesis as a light client — cheap, and still checking that more than two thirds of
//! the stake made each one final. It takes the ledger as it stood at the last of them from
//! anybody at all, as a snapshot (`Ledger::snapshot`), and believes it exactly as far as it
//! hashes to the state root that header commits to. And it replays in full only the blocks
//! after.
//!
//! What it needs besides is who proposes when. Headers commit to who is in each validator set
//! and with what stake, not to where each stands in the rotation, which is derived; so the
//! joiner turns the rotation itself as it follows, exactly as the chain did, and checks where it
//! arrives against the snapshot's own validators and the last header's word for who signs next.
//! A joiner that got that wrong would not be fooled — it would refuse the real chain's next
//! block for its proposer — but it would be stuck.

use crate::block::Block;
use crate::consensus::ValidatorSet;
use crate::light::{LightBlock, follow};
use crate::node::{Genesis, Invalid, made_final, next_state};
use crate::state::Ledger;
use crate::tx::Address;
use crate::PublicKey;

/// Join a chain at the last of `headers`: follow them from the genesis, take the ledger there
/// from `snapshot`, and replay `after` — every block since, in full — from it. Returns the ledger
/// at the end, which is the chain's to the byte, or the height at which something fails and why.
pub fn join(
    genesis: &Genesis,
    headers: &[LightBlock],
    snapshot: &[Vec<u8>],
    after: &[Block],
) -> Result<Ledger, (u64, Invalid)> {
    let height = follow(genesis, headers)?;
    let id = genesis.id();
    let last = &headers[headers.len() - 1];

    // The rotation through the heights before the last, turned as the chain turned it: each
    // height's proposer takes their turns, and the set that signs next — which the next light
    // block carries, checked against this header — takes over with the priorities that leaves.
    let founding = Ledger::genesis(id, genesis.params.clone(), &genesis.allocations);
    let mut rotation = ValidatorSet::after(&ValidatorSet::default(), &founding.validators());
    for pair in headers[1..].windows(2) {
        let (here, next) = (&pair[0], &pair[1]);
        let (turned, _) = rotation.at_round(here.header.round);
        rotation = ValidatorSet::after(&turned, &members(&next.validators));
    }

    let mut ledger = Ledger::from_snapshot(id, genesis.params.clone(), snapshot)
        .map_err(|why| (height, Invalid::BadSnapshot(why)))?;
    if ledger.root() != last.header.state {
        return Err((height, Invalid::WrongStateRoot));
    }
    // And the last height's own turn, handing over to the validators the snapshot says come
    // next — who must be the ones the header said would.
    rotation = if height == 0 {
        ValidatorSet::after(&ValidatorSet::default(), &ledger.validators())
    } else {
        ValidatorSet::after(&rotation.at_round(last.header.round).0, &ledger.validators())
    };
    if rotation.hash() != last.header.next_validators {
        return Err((height, Invalid::WrongValidators));
    }

    let trusted = Block {
        header: last.header.clone(),
        txs: Vec::new(),
        commit: last.commit.clone(),
    };
    let mut tip = &trusted;
    for block in after {
        let at = block.header.height;
        let (next_ledger, next_rotation) =
            next_state(&ledger, &rotation, tip, id, block, None).map_err(|why| (at, why))?;
        made_final(&rotation, &block.commit, &block.header).map_err(|why| (at, why))?;
        ledger = next_ledger;
        rotation = next_rotation;
        tip = block;
    }
    Ok(ledger)
}

/// Who is in a set and with what stake: all its hash commits to.
fn members(set: &ValidatorSet) -> Vec<(Address, PublicKey, u64)> {
    set.members.iter().map(|v| (v.address, v.key, v.power)).collect()
}
