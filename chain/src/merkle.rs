//! Merkle trees, as RFC 6962 builds them.
//!
//! A block cannot carry its transactions inside its header — a header is what validators sign
//! and what the next block names, so it has to be small — but it has to *commit* to them, so
//! that nobody can later swap one transaction for another and keep the signatures. A Merkle
//! root is that commitment: thirty-two bytes that change if any transaction changes, and from
//! which anybody can be shown that one particular transaction is in the block with a handful
//! of hashes rather than the whole block.
//!
//! RFC 6962's version rather than Bitcoin's, for one reason worth stating. Bitcoin pads an odd
//! level by duplicating its last node, which means a block of transactions `[a, b, c]` and a
//! block of `[a, b, c, c]` have the same root — a real bug, CVE-2012-2459, that let a node be
//! fed an invalid block with a valid root. RFC 6962 splits at the largest power of two instead
//! and never duplicates anything, and it prefixes leaves and interior nodes with different
//! bytes, so a leaf can never be passed off as a node.

use crate::Digest;
use crate::sha2::Sha256;

/// The hash of one leaf: SHA-256(0x00 ‖ data).
pub fn leaf(data: &[u8]) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(&[0x00]);
    hasher.update(data);
    Digest(hasher.finish())
}

/// The hash of an interior node: SHA-256(0x01 ‖ left ‖ right).
pub fn node(left: &Digest, right: &Digest) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(&[0x01]);
    hasher.update(&left.0);
    hasher.update(&right.0);
    Digest(hasher.finish())
}

/// The largest power of two strictly below `n`, for `n` of at least two.
fn split(n: usize) -> usize {
    debug_assert!(n >= 2);
    let mut k = 1;
    while k * 2 < n {
        k *= 2;
    }
    k
}

/// The root of a tree over leaves that have already been through [`leaf`].
///
/// The empty tree's root is the hash of nothing, as the RFC defines it.
pub fn root(leaves: &[Digest]) -> Digest {
    match leaves.len() {
        0 => Digest(crate::sha2::sha256(b"")),
        1 => leaves[0],
        n => {
            let k = split(n);
            node(&root(&leaves[..k]), &root(&leaves[k..]))
        }
    }
}

/// The audit path for one leaf: the sibling hashes from the leaf up to the root, nearest
/// first. RFC 6962 §2.1.1.
pub fn proof(leaves: &[Digest], index: usize) -> Vec<Digest> {
    assert!(index < leaves.len(), "no leaf {index} in a tree of {}", leaves.len());
    let mut path = Vec::new();
    path_into(leaves, index, &mut path);
    path
}

fn path_into(leaves: &[Digest], index: usize, path: &mut Vec<Digest>) {
    let n = leaves.len();
    if n <= 1 {
        return;
    }
    let k = split(n);
    if index < k {
        path_into(&leaves[..k], index, path);
        path.push(root(&leaves[k..]));
    } else {
        path_into(&leaves[k..], index - k, path);
        path.push(root(&leaves[..k]));
    }
}

/// Whether `leaf` is leaf number `index` of a tree of `size` leaves with this `root`.
///
/// RFC 9162 §2.1.3.2, which walks the path using nothing but the index and the size — the
/// verifier never needs to see any other leaf.
pub fn verify(root: &Digest, leaf: &Digest, index: usize, size: usize, path: &[Digest]) -> bool {
    if index >= size {
        return false;
    }
    let (mut f, mut s) = (index, size - 1);
    let mut r = *leaf;
    for sibling in path {
        if s == 0 {
            return false;
        }
        if f & 1 == 1 || f == s {
            r = node(sibling, &r);
            if f & 1 == 0 {
                while f & 1 == 0 && f != 0 {
                    f >>= 1;
                    s >>= 1;
                }
            }
        } else {
            r = node(&r, sibling);
        }
        f >>= 1;
        s >>= 1;
    }
    s == 0 && r == *root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hex;

    /// The Certificate Transparency reference tree: eight leaves, and the root of every prefix.
    fn reference_leaves() -> Vec<Digest> {
        let raw: [&[u8]; 8] = [
            b"",
            b"\x00",
            b"\x10",
            b"\x20\x21",
            b"\x30\x31",
            b"\x40\x41\x42\x43",
            b"\x50\x51\x52\x53\x54\x55\x56\x57",
            b"\x60\x61\x62\x63\x64\x65\x66\x67\x68\x69\x6a\x6b\x6c\x6d\x6e\x6f",
        ];
        raw.iter().map(|d| leaf(d)).collect()
    }

    #[test]
    fn roots_match_the_certificate_transparency_reference() {
        let expected = [
            "6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d",
            "fac54203e7cc696cf0dfcb42c92a1d9dbaf70ad9e621f4bd8d98662f00e3c125",
            "aeb6bcfe274b70a14fb067a5e5578264db0fa9b51af5e0ba159158f329e06e77",
            "d37ee418976dd95753c1c73862b9398fa2a2cf9b4ff0fdfe8b30cd95209614b7",
            "4e3bbb1f7b478dcfe71fb631631519a3bca12c9aefca1612bfce4c13a86264d4",
            "76e67dadbcdf1e10e1b74ddc608abd2f98dfb16fbce75277b5232a127f2087ef",
            "ddb89be403809e325750d3d263cd78929c2942b7942a34b77e122c9594a74c8c",
            "5dc9da79a70659a9ad559cb701ded9a2ab9d823aad2f4960cfe370eff4604328",
        ];
        let leaves = reference_leaves();
        for (n, want) in expected.iter().enumerate() {
            assert_eq!(hex(&root(&leaves[..n + 1]).0), *want, "root of the first {}", n + 1);
        }
        assert_eq!(
            hex(&root(&[]).0),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// Every leaf of every size proves, and proves nothing else.
    #[test]
    fn every_leaf_proves_its_place_and_no_other() {
        let leaves: Vec<Digest> = (0..37u32).map(|i| leaf(&i.to_le_bytes())).collect();
        for size in 1..=leaves.len() {
            let tree = &leaves[..size];
            let top = root(tree);
            for index in 0..size {
                let path = proof(tree, index);
                assert!(verify(&top, &tree[index], index, size, &path), "{index} of {size}");
                // Not at another position, not as another leaf, not in a tree of another size.
                if size > 1 {
                    let elsewhere = (index + 1) % size;
                    assert!(!verify(&top, &tree[index], elsewhere, size, &path));
                    assert!(!verify(&top, &tree[elsewhere], index, size, &path));
                    // A path with any hash in it changed, or with one missing, proves nothing.
                    for at in 0..path.len() {
                        let mut bent = path.clone();
                        bent[at].0[0] ^= 1;
                        assert!(!verify(&top, &tree[index], index, size, &bent));
                    }
                    assert!(!verify(&top, &tree[index], index, size, &path[..path.len() - 1]));
                }
                // What a proof does *not* do is pin the size on its own: leaf 0 of three and
                // leaf 0 of four walk the same way, because a sibling is a hash and a verifier
                // cannot tell a leaf's from a subtree's. That is why a header commits to the
                // root and the count together, and why a proof is checked against both.
            }
        }
    }

    /// The bug RFC 6962's shape exists to avoid: `[a, b, c]` and `[a, b, c, c]` must not share
    /// a root.
    #[test]
    fn repeating_the_last_leaf_changes_the_root() {
        let a = leaf(b"a");
        let b = leaf(b"b");
        let c = leaf(b"c");
        assert_ne!(root(&[a, b, c]), root(&[a, b, c, c]));
        // And a leaf cannot be passed off as the node above two others.
        assert_ne!(node(&a, &b), leaf(&[a.0, b.0].concat()));
    }
}
