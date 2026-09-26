//! What somebody asks the ledger to do, and their signature on it.
//!
//! An account model rather than unspent outputs: a balance per address per asset, and a
//! **nonce** per address that every transaction must match exactly and that advances by one
//! when it is applied. The nonce is what makes a transaction unrepeatable — the same signed
//! payment submitted twice is valid once and refused the second time, because by then the
//! account has moved on — and the chain id folded into what is signed is what makes it
//! unrepeatable on any *other* chain.
//!
//! Two kinds of money live here and they are different on purpose:
//!
//! - **The coin** is the chain's own. It is created by the chain, as the reward for keeping
//!   it, and it is what fees are paid in and what validators stake. Nothing backs it; it is
//!   worth what anybody will give for it.
//! - **A stable token** is somebody's promise. An issuer registers it, names the currency it
//!   stands for and an independent attestor who will vouch for the reserve behind it, and can
//!   then mint only as much as the attestor has most recently said is held. Holders redeem by
//!   handing tokens back, which destroys them. What keeps the token worth one unit of its
//!   currency is that anybody can make exactly that trade with the issuer — and what keeps
//!   the issuer honest is that the reserve and the supply are both on the ledger for anybody
//!   to compare.

use crate::codec::Writer;
use crate::consensus::Vote;
use crate::{Digest, PublicKey, Signature, SigningKey};

/// Base units in one coin. Eight decimal places, so that a fee can be a small fraction of a
/// coin and still be an integer. Nothing on the ledger is ever a float.
pub const COIN: u128 = 100_000_000;

/// Base units in one unit of the currency a stable token stands for: six decimal places, as
/// the large fiat-backed tokens use.
pub const TOKEN_UNIT: u128 = 1_000_000;

/// Where value lives: the first twenty bytes of a hash of somebody's public key.
///
/// A hash rather than the key itself so that anybody can be paid before they have ever
/// signed anything, and so that the key appears on the ledger only once its owner spends.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Address(pub [u8; 20]);

impl Address {
    pub fn of(key: &PublicKey) -> Address {
        let digest = Digest::of(&Writer::tagged("life-rs/chain/address/1").fixed(&key.0).finish());
        let mut out = [0u8; 20];
        out.copy_from_slice(&digest.0[..20]);
        Address(out)
    }

    /// The first few bytes, which is how a person reads one.
    pub fn short(&self) -> String {
        crate::hex(&self.0[..4])
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", crate::hex(&self.0))
    }
}

impl std::fmt::Debug for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Address({})", self.short())
    }
}

/// Which money.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Asset {
    /// The chain's own coin.
    Coin,
    /// A stable token, by the order in which it was registered.
    Token(u32),
}

/// What a transaction does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Move coin or tokens to somebody else.
    Pay {
        to: Address,
        asset: Asset,
        amount: u128,
    },
    /// Register a stable token issued by the sender, standing for one unit of `peg`, whose
    /// reserve `attestor` will vouch for.
    Issue {
        symbol: String,
        peg: String,
        attestor: Address,
    },
    /// The attestor's statement of what the issuer holds in reserve, in base units.
    Attest { token: u32, reserves: u128 },
    /// The issuer creates tokens for somebody who has paid it the currency they stand for.
    Mint { token: u32, to: Address, amount: u128 },
    /// A holder hands tokens back to be exchanged for their currency. They are destroyed.
    Redeem { token: u32, amount: u128 },
    /// Lock coin as stake, to take a share of keeping the chain.
    Bond { amount: u128 },
    /// Unlock it again.
    Unbond { amount: u128 },
    /// Two votes by one validator for two different blocks at the same height and round.
    ///
    /// That is the one thing a validator can do that is provably dishonest from the outside,
    /// and it is what makes stake worth staking: double-signing is how a chain is forked, so
    /// it is what costs a validator their stake.
    Evidence { first: Box<Vote>, second: Box<Vote> },
}

impl Action {
    /// A word for what it is.
    pub fn label(&self) -> &'static str {
        match self {
            Action::Pay { .. } => "pay",
            Action::Issue { .. } => "issue",
            Action::Attest { .. } => "attest",
            Action::Mint { .. } => "mint",
            Action::Redeem { .. } => "redeem",
            Action::Bond { .. } => "bond",
            Action::Unbond { .. } => "unbond",
            Action::Evidence { .. } => "evidence",
        }
    }

    fn encode_into(&self, w: &mut Writer) {
        match self {
            Action::Pay { to, asset, amount } => {
                w.u8(0).fixed(&to.0);
                match asset {
                    Asset::Coin => w.u8(0),
                    Asset::Token(id) => w.u8(1).u32(*id),
                };
                w.u128(*amount);
            }
            Action::Issue {
                symbol,
                peg,
                attestor,
            } => {
                w.u8(1).text(symbol).text(peg).fixed(&attestor.0);
            }
            Action::Attest { token, reserves } => {
                w.u8(2).u32(*token).u128(*reserves);
            }
            Action::Mint { token, to, amount } => {
                w.u8(3).u32(*token).fixed(&to.0).u128(*amount);
            }
            Action::Redeem { token, amount } => {
                w.u8(4).u32(*token).u128(*amount);
            }
            Action::Bond { amount } => {
                w.u8(5).u128(*amount);
            }
            Action::Unbond { amount } => {
                w.u8(6).u128(*amount);
            }
            Action::Evidence { first, second } => {
                w.u8(7).var(&first.encode()).var(&second.encode());
            }
        }
    }
}

/// A signed request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    /// Which chain this is for — the hash of its genesis.
    pub chain: Digest,
    pub signer: PublicKey,
    /// Must equal the signer's account nonce when applied.
    pub nonce: u64,
    /// In coin base units, to the validator who proposes the block it lands in.
    pub fee: u128,
    pub action: Action,
    pub signature: Signature,
}

impl Transaction {
    /// Write, and sign.
    pub fn signed(key: &SigningKey, chain: Digest, nonce: u64, fee: u128, action: Action) -> Transaction {
        let mut tx = Transaction {
            chain,
            signer: key.public(),
            nonce,
            fee,
            action,
            signature: Signature([0; 64]),
        };
        tx.signature = key.sign(&tx.body());
        tx
    }

    /// Everything that is signed: all of it but the signature.
    pub fn body(&self) -> Vec<u8> {
        let mut w = Writer::tagged("life-rs/chain/tx/1");
        w.fixed(&self.chain.0)
            .fixed(&self.signer.0)
            .u64(self.nonce)
            .u128(self.fee);
        self.action.encode_into(&mut w);
        w.finish()
    }

    /// The whole of it, signature included — which is what goes into a block.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = self.body();
        bytes.extend_from_slice(&self.signature.0);
        bytes
    }

    /// What the transaction is known by.
    ///
    /// Unique because a signature is: Ed25519 is deterministic and an unreduced S is refused,
    /// so a given signer signing a given body produces exactly one valid transaction.
    pub fn id(&self) -> Digest {
        Digest::of(&self.encode())
    }

    pub fn sender(&self) -> Address {
        Address::of(&self.signer)
    }

    pub fn signature_holds(&self) -> bool {
        self.signer.verify(&self.body(), &self.signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u8) -> SigningKey {
        SigningKey::from_seed([n; 32])
    }

    #[test]
    fn a_transaction_is_signed_by_its_signer_and_nobody_else() {
        let alice = key(1);
        let bob = Address::of(&key(2).public());
        let chain = Digest::of(b"a chain");
        let pay = Transaction::signed(
            &alice,
            chain,
            0,
            10,
            Action::Pay {
                to: bob,
                asset: Asset::Coin,
                amount: 5 * COIN,
            },
        );
        assert!(pay.signature_holds());

        // Change anything that was signed and the signature stops holding.
        let mut more = pay.clone();
        if let Action::Pay { amount, .. } = &mut more.action {
            *amount += 1;
        }
        assert!(!more.signature_holds(), "an amount changed after signing");
        let mut elsewhere = pay.clone();
        elsewhere.chain = Digest::of(b"another chain");
        assert!(!elsewhere.signature_holds(), "replayed on another chain");
        let mut again = pay.clone();
        again.nonce += 1;
        assert!(!again.signature_holds(), "replayed at another nonce");
        let mut claimed = pay.clone();
        claimed.signer = key(3).public();
        assert!(!claimed.signature_holds(), "claimed by another key");
    }

    #[test]
    fn two_different_transactions_have_two_different_ids() {
        let alice = key(1);
        let chain = Digest::of(b"a chain");
        let bond = |amount| Transaction::signed(&alice, chain, 0, 0, Action::Bond { amount });
        assert_eq!(bond(5).id(), bond(5).id(), "signing is deterministic");
        assert_ne!(bond(5).id(), bond(6).id());
    }
}
