//! A chain as a file: its genesis and every block — everything anybody needs to check it, and
//! nothing they need to trust.
//!
//! The file is written in the chain's own canonical encoding, so reading it is as strict as
//! signing: whatever `read` accepts, `write` writes back to the same bytes, and there is no
//! second way to write the same chain. What reading does not do is believe anything. A file
//! that reads is only a claim to be a chain; `Chain::replay` checks the claim from the genesis,
//! every signature and root again, and a file changed anywhere — one bit of one vote — either
//! does not read or does not replay.

use crate::codec::{Malformed, Reader, Writer};
use crate::{Block, Genesis};

const TAG: &str = "life-rs/chain/file/1";

/// Why a file is not a chain's, and where: in the genesis, or in which block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unreadable {
    /// The index of the block that does not read, or `None` for the file's own framing and the
    /// genesis.
    pub block: Option<usize>,
    pub why: Malformed,
}

/// The file for a chain.
pub fn write(genesis: &Genesis, blocks: &[Block]) -> Vec<u8> {
    let mut w = Writer::tagged(TAG);
    w.var(&genesis.encode()).u32(blocks.len() as u32);
    for block in blocks {
        w.var(&block.encode());
    }
    w.finish()
}

/// The genesis and blocks a file holds, or where it stops being one.
pub fn read(bytes: &[u8]) -> Result<(Genesis, Vec<Block>), Unreadable> {
    let framing = |why| Unreadable { block: None, why };
    let mut r = Reader::tagged(bytes, TAG).map_err(framing)?;
    let genesis = r.var().and_then(Genesis::decode).map_err(framing)?;
    let n = r.count(4).map_err(framing)?;
    let mut blocks = Vec::with_capacity(n);
    for at in 0..n {
        let block = r
            .var()
            .and_then(Block::decode)
            .map_err(|why| Unreadable { block: Some(at), why })?;
        blocks.push(block);
    }
    r.done().map_err(framing)?;
    Ok((genesis, blocks))
}
