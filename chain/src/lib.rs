//! A ledger nobody keeps.
//!
//! Everything else in this workspace is a thing the world is made of. This is a thing the
//! world's people make, late, once they need it: a record of who owns what that no single one
//! of them is trusted to hold, kept by several parties who do not trust each other, in a form
//! where any of them — or anybody reading afterwards — can check every entry without taking
//! anybody's word for it.
//!
//! It is a blockchain, and it is a real one. The standard for "real" here is the same one
//! design principle five sets for the planet: **the mechanism is real and the resolution is
//! coarse.** Every block names its parent by SHA-256; every transaction is signed with
//! Ed25519 and refused if the signature does not verify; every block commits to a Merkle root
//! of its transactions and of the whole ledger after them; and a block is only a block once
//! validators holding more than two thirds of the stake have signed it. Change one byte
//! anywhere in the history and the change is visible from every later block. Nothing is
//! simulated *about* the chain — what is coarse is how often a block is cut, which is once a
//! month of the world's time rather than once every few seconds.
//!
//! ## Why stake and not work
//!
//! Proof of work is the famous one, and it is the wrong one to *simulate*. Its security is
//! the real cost of the hashing, so a simulated world whose miners grow a thousandfold must
//! either make the machine running it hash a thousand times more per block, or quietly let
//! the difficulty written in a header stop being the work that was actually done — at which
//! point the chain is a picture of a chain. A committee's signatures have no such problem:
//! checking that validators with two thirds of the stake signed a block costs the same inside
//! a simulation as outside it, and it is exactly as convincing in both.
//!
//! So this is Byzantine-fault-tolerant proof of stake in the Tendermint family: a proposer
//! chosen by weighted rotation, precommit signatures from the validator set, and a block that
//! is final the moment it is committed. It stays safe with fewer than a third of the stake
//! faulty and stays live with more than two thirds of it answering.
//!
//! ## What is here
//!
//! - `sha2` and `ed25519` — the two primitives, written out from their standards and checked
//!   against the published vectors and against OpenSSL.
//! - `merkle` — RFC 6962 trees, with inclusion proofs.
//! - `codec` — one canonical encoding, with a domain tag on everything signed or hashed.
//! - `tx` and `state` — an account ledger with a native coin and stable tokens that can only
//!   be minted against an independently attested reserve, and swaps of one for the other that
//!   settle both legs at once or neither.
//! - `consensus`, `block` and `node` — validator sets, weighted proposer rotation, commits of
//!   more than two thirds of the stake, and a chain that can be replayed from genesis by
//!   anybody, with nothing taken on trust.
//! - `light` — following a chain by its headers alone, each commit checked against the set the
//!   header before handed over to, for somebody who wants to believe a header without keeping
//!   the ledger behind it.
//! - `file` — a chain as a file, in the same canonical encoding and read back as strictly: its
//!   genesis and every block, which is all anybody needs to replay it somewhere else.
//!
//! ## What is not
//!
//! A network. Every node of this chain lives in one process and hears every message at once,
//! so there is no gossip, no latency and no partition — a validator is either answering this
//! round or it is not. That is the coarse part, and it is where a partition would go.

pub mod block;
pub mod codec;
pub mod consensus;
pub mod ed25519;
pub mod file;
pub mod light;
pub mod merkle;
pub mod node;
pub mod sha2;
pub mod state;
pub mod tx;

pub use block::{Block, Header};
pub use consensus::{Commit, ValidatorSet, Vote};
pub use ed25519::{PublicKey, Signature, SigningKey};
pub use light::{Fork, LightBlock, follow, follow_from, fork};
pub use node::{Chain, Committed, Genesis, Invalid};
pub use state::{Ledger, MAX_SUPPLY, MAX_TOKENS, Params, Refusal, Token};
pub use tx::{Action, Address, Asset, COIN, Swap, TOKEN_UNIT, Transaction};

/// Thirty-two bytes of SHA-256: the name of a block, of a transaction, or of a tree of either.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Digest(pub [u8; 32]);

impl Digest {
    /// All zeros — the parent of the first block, which has none.
    pub const ZERO: Digest = Digest([0; 32]);

    /// SHA-256 of some bytes.
    pub fn of(data: &[u8]) -> Digest {
        Digest(sha2::sha256(data))
    }

    /// The first few bytes, which is how a person reads a hash.
    pub fn short(&self) -> String {
        hex(&self.0[..6])
    }
}

impl std::fmt::Display for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex(&self.0))
    }
}

impl std::fmt::Debug for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Digest({}…)", self.short())
    }
}

/// Lower-case hex, for reading.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 15) as usize] as char);
    }
    out
}

/// Hex back to bytes. Panics on anything that is not hex, because it is only ever handed
/// literals.
pub fn unhex(text: &str) -> Vec<u8> {
    assert!(text.len().is_multiple_of(2), "hex has two digits a byte");
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex digits"))
        .collect()
}

#[cfg(test)]
mod tests;
