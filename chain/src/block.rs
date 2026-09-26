//! A block: a header everybody signs, the transactions it commits to, and the signatures.

use crate::Digest;
use crate::codec::Writer;
use crate::consensus::Commit;
use crate::tx::{Address, Transaction};

/// What validators sign and what the next block names.
///
/// Everything a reader needs to check a block is committed to here by hash: the parent, the
/// transactions, the ledger they produce, who must sign this block and who must sign the next,
/// and the signatures that made the parent final. Change any of it and the hash changes, and
/// with it every vote for this block and the parent hash of every block after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub chain: Digest,
    pub height: u64,
    /// Which attempt at this height produced it. Zero unless a proposer failed to show.
    pub round: u32,
    /// Seconds since the world's founding.
    pub time: u64,
    pub parent: Digest,
    /// The commit that made the parent final.
    pub last_commit: Digest,
    /// Merkle root of the transactions' ids.
    pub txs: Digest,
    pub tx_count: u32,
    /// The state root after applying them.
    pub state: Digest,
    pub validators: Digest,
    pub next_validators: Digest,
    pub proposer: Address,
}

impl Header {
    pub fn encode(&self) -> Vec<u8> {
        Writer::tagged("life-rs/chain/header/1")
            .fixed(&self.chain.0)
            .u64(self.height)
            .u32(self.round)
            .u64(self.time)
            .fixed(&self.parent.0)
            .fixed(&self.last_commit.0)
            .fixed(&self.txs.0)
            .u32(self.tx_count)
            .fixed(&self.state.0)
            .fixed(&self.validators.0)
            .fixed(&self.next_validators.0)
            .fixed(&self.proposer.0)
            .finish()
    }

    pub fn hash(&self) -> Digest {
        Digest::of(&self.encode())
    }
}

/// A committed block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub header: Header,
    pub txs: Vec<Transaction>,
    /// The votes that made it final. Not part of the hash — they are signatures *of* the hash —
    /// but committed to by the next block's `last_commit`.
    pub commit: Commit,
}

impl Block {
    pub fn hash(&self) -> Digest {
        self.header.hash()
    }

    /// The Merkle root of some transactions, as a header commits to it.
    pub fn root_of(txs: &[Transaction]) -> Digest {
        let leaves: Vec<Digest> = txs.iter().map(|t| crate::merkle::leaf(&t.id().0)).collect();
        crate::merkle::root(&leaves)
    }
}
