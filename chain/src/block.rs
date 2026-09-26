//! A block: a header everybody signs, the transactions it commits to, and the signatures.

use crate::Digest;
use crate::codec::{Malformed, Reader, Writer};
use crate::consensus::{Commit, Vote};
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

    /// `encode`, backwards.
    pub fn decode(bytes: &[u8]) -> Result<Header, Malformed> {
        let mut r = Reader::tagged(bytes, "life-rs/chain/header/1")?;
        let header = Header {
            chain: Digest(r.fixed()?),
            height: r.u64()?,
            round: r.u32()?,
            time: r.u64()?,
            parent: Digest(r.fixed()?),
            last_commit: Digest(r.fixed()?),
            txs: Digest(r.fixed()?),
            tx_count: r.u32()?,
            state: Digest(r.fixed()?),
            validators: Digest(r.fixed()?),
            next_validators: Digest(r.fixed()?),
            proposer: Address(r.fixed()?),
        };
        r.done()?;
        Ok(header)
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

    /// The whole block, as it is kept and handed on: the header, every transaction and the
    /// votes that made it final, each in its own encoding.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::tagged("life-rs/chain/block/1");
        w.var(&self.header.encode()).u32(self.txs.len() as u32);
        for tx in &self.txs {
            w.var(&tx.encode());
        }
        w.u32(self.commit.round).u32(self.commit.votes.len() as u32);
        for vote in &self.commit.votes {
            w.var(&vote.encode());
        }
        w.finish()
    }

    /// `encode`, backwards. A block that reads is only a claim to be one; `Chain::replay`
    /// checks the claim.
    pub fn decode(bytes: &[u8]) -> Result<Block, Malformed> {
        let mut r = Reader::tagged(bytes, "life-rs/chain/block/1")?;
        let header = Header::decode(r.var()?)?;
        let n = r.count(4)?;
        let mut txs = Vec::with_capacity(n);
        for _ in 0..n {
            txs.push(Transaction::decode(r.var()?)?);
        }
        let round = r.u32()?;
        let n = r.count(4)?;
        let mut votes = Vec::with_capacity(n);
        for _ in 0..n {
            votes.push(Vote::decode(r.var()?)?);
        }
        r.done()?;
        Ok(Block {
            header,
            txs,
            commit: Commit { round, votes },
        })
    }

    /// The Merkle root of some transactions, as a header commits to it.
    pub fn root_of(txs: &[Transaction]) -> Digest {
        let leaves: Vec<Digest> = txs.iter().map(|t| crate::merkle::leaf(&t.id().0)).collect();
        crate::merkle::root(&leaves)
    }

    /// A receipt for the transaction at `index`: it, and the path from it to the root the
    /// header commits to.
    pub fn receipt(&self, index: usize) -> Option<Receipt> {
        let tx = self.txs.get(index)?.clone();
        let leaves: Vec<Digest> = self.txs.iter().map(|t| crate::merkle::leaf(&t.id().0)).collect();
        Some(Receipt {
            tx,
            height: self.header.height,
            index,
            size: self.txs.len(),
            path: crate::merkle::proof(&leaves, index),
        })
    }
}

/// That a transaction was carried: the transaction, where it sits in its block, and the path from
/// it to the root the block's header commits to. Checked against the header alone — which is what
/// a light client holds, having checked that more than two thirds of the stake made it final — it
/// proves a payment to somebody who keeps no copy of the chain. A block carries nothing that does
/// not apply, so carried is done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub tx: Transaction,
    pub height: u64,
    pub index: usize,
    pub size: usize,
    pub path: Vec<Digest>,
}

impl Receipt {
    /// Whether this is a receipt for a transaction in the block this header heads: signed by the
    /// key it names, at its place among as many transactions as the header says the block holds.
    pub fn holds_under(&self, header: &Header) -> bool {
        header.height == self.height
            && header.tx_count as usize == self.size
            && self.tx.chain == header.chain
            && self.tx.signature_holds()
            && crate::merkle::verify(
                &header.txs,
                &crate::merkle::leaf(&self.tx.id().0),
                self.index,
                self.size,
                &self.path,
            )
    }
}
