//! Ed25519 signatures, as RFC 8032 writes them down.
//!
//! A ledger nobody keeps has exactly one way of knowing that the person spending a balance is
//! the person it belongs to: they can produce a signature nobody else could have produced.
//! Everything else on a chain — the order of blocks, the commit of a validator, the reserve an
//! issuer claims to hold — is also only as good as this, so it is written out in full rather
//! than approximated, and checked against the RFC's own test vectors and against OpenSSL.
//!
//! ## What is here
//!
//! - **The field** of integers modulo p = 2²⁵⁵ − 19, in five limbs of 51 bits, which is the
//!   representation almost every serious implementation uses because a product of two limbs
//!   fits in a `u128` with room to spare.
//! - **The curve**, −x² + y² = 1 + d·x²·y², in extended coordinates, where addition is
//!   *complete*: the same formula adds any two points, including a point to itself and to the
//!   identity, so there are no special cases for a caller to forget.
//! - **Scalars** modulo the group order L = 2²⁵² + 27742317777372353535851937790883648493.
//! - **Keys, signing and verifying**, deterministically: the nonce is a hash of the key and the
//!   message, so the same key signing the same message twice gives the same signature and no
//!   random-number generator is ever consulted. That is the property that lets a world signed
//!   by seeded keys replay exactly.
//!
//! ## What is not
//!
//! **Constant time.** Nothing here tries to hide which branch it took or how long it spent,
//! because nothing here has a secret an attacker could time: every key in this simulation is
//! derived from a world seed printed at the top of every run. Do not lift this into anything
//! that holds a real key.

use crate::sha2::Sha512;
use std::sync::OnceLock;

const MASK51: u64 = (1 << 51) - 1;

/// An element of the field of integers modulo 2²⁵⁵ − 19.
///
/// Five limbs of 51 bits. The limbs are allowed to run a little over 51 bits between
/// reductions — sums of two reduced elements, never more — which is what lets addition skip
/// carrying altogether.
#[derive(Clone, Copy, Debug)]
struct Fe([u64; 5]);

/// −121665/121666, the constant that picks out this curve.
const D: Fe = Fe([
    0x34dca135978a3,
    0x1a8283b156ebd,
    0x5e7a26001c029,
    0x739c663a03cbb,
    0x52036cee2b6ff,
]);
/// 2·d, which is what the addition formula actually uses.
const D2: Fe = Fe([
    0x69b9426b2f159,
    0x35050762add7a,
    0x3cf44c0038052,
    0x6738cc7407977,
    0x2406d9dc56dff,
]);
/// A square root of −1, which exists because p ≡ 5 (mod 8).
const SQRT_M1: Fe = Fe([
    0x61b274a0ea0b0,
    0xd5a5fc8f189d,
    0x7ef5e9cbd0c60,
    0x78595a6804c9e,
    0x2b8324804fc1d,
]);

impl Fe {
    const ZERO: Fe = Fe([0; 5]);
    const ONE: Fe = Fe([1, 0, 0, 0, 0]);

    /// Little-endian, with the top bit ignored — the top bit of an encoded point is the sign of
    /// its x coordinate, not part of y.
    fn from_bytes(bytes: &[u8; 32]) -> Fe {
        let load = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().expect("8 bytes"));
        Fe([
            load(0) & MASK51,
            (load(6) >> 3) & MASK51,
            (load(12) >> 6) & MASK51,
            (load(19) >> 1) & MASK51,
            (load(24) >> 12) & MASK51,
        ])
    }

    /// The canonical encoding: fully reduced below p, little-endian.
    fn to_bytes(self) -> [u8; 32] {
        let mut t = self.weak_reduce().0;
        // After a weak reduction the value is below 2p, so it is either already reduced or
        // exactly one p too large — and it is too large precisely when adding 19 carries out
        // of the 255th bit.
        let mut q = (t[0] + 19) >> 51;
        q = (t[1] + q) >> 51;
        q = (t[2] + q) >> 51;
        q = (t[3] + q) >> 51;
        q = (t[4] + q) >> 51;
        t[0] += 19 * q;
        t[1] += t[0] >> 51;
        t[0] &= MASK51;
        t[2] += t[1] >> 51;
        t[1] &= MASK51;
        t[3] += t[2] >> 51;
        t[2] &= MASK51;
        t[4] += t[3] >> 51;
        t[3] &= MASK51;
        // The carry out of the top limb is the 2²⁵⁵ being subtracted; dropping it is the
        // subtraction.
        t[4] &= MASK51;
        let words = [
            t[0] | (t[1] << 51),
            (t[1] >> 13) | (t[2] << 38),
            (t[2] >> 26) | (t[3] << 25),
            (t[3] >> 39) | (t[4] << 12),
        ];
        let mut out = [0u8; 32];
        for (chunk, word) in out.chunks_exact_mut(8).zip(words) {
            chunk.copy_from_slice(&word.to_le_bytes());
        }
        out
    }

    /// Bring every limb back to about 51 bits without making the value canonical.
    fn weak_reduce(self) -> Fe {
        let mut l = self.0;
        let carries = [l[0] >> 51, l[1] >> 51, l[2] >> 51, l[3] >> 51, l[4] >> 51];
        for limb in &mut l {
            *limb &= MASK51;
        }
        // What carries out of the top wraps round to the bottom times nineteen, because
        // 2²⁵⁵ ≡ 19.
        l[0] += carries[4] * 19;
        l[1] += carries[0];
        l[2] += carries[1];
        l[3] += carries[2];
        l[4] += carries[3];
        Fe(l)
    }

    fn add(self, other: Fe) -> Fe {
        let (a, b) = (self.0, other.0);
        Fe([a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3], a[4] + b[4]])
    }

    fn sub(self, other: Fe) -> Fe {
        // Sixteen p added first, which is larger than anything a limb can hold here, so the
        // subtraction cannot go below zero.
        let (a, b) = (self.0, other.0);
        Fe([
            (a[0] + 36_028_797_018_963_664) - b[0],
            (a[1] + 36_028_797_018_963_952) - b[1],
            (a[2] + 36_028_797_018_963_952) - b[2],
            (a[3] + 36_028_797_018_963_952) - b[3],
            (a[4] + 36_028_797_018_963_952) - b[4],
        ])
        .weak_reduce()
    }

    fn neg(self) -> Fe {
        Fe::ZERO.sub(self)
    }

    fn mul(self, other: Fe) -> Fe {
        #[inline(always)]
        fn m(x: u64, y: u64) -> u128 {
            (x as u128) * (y as u128)
        }
        let (a, b) = (self.0, other.0);
        // Limbs that wrap past the top come back round multiplied by nineteen.
        let (b1, b2, b3, b4) = (b[1] * 19, b[2] * 19, b[3] * 19, b[4] * 19);
        let c0 = m(a[0], b[0]) + m(a[4], b1) + m(a[3], b2) + m(a[2], b3) + m(a[1], b4);
        let mut c1 = m(a[1], b[0]) + m(a[0], b[1]) + m(a[4], b2) + m(a[3], b3) + m(a[2], b4);
        let mut c2 = m(a[2], b[0]) + m(a[1], b[1]) + m(a[0], b[2]) + m(a[4], b3) + m(a[3], b4);
        let mut c3 = m(a[3], b[0]) + m(a[2], b[1]) + m(a[1], b[2]) + m(a[0], b[3]) + m(a[4], b4);
        let mut c4 = m(a[4], b[0]) + m(a[3], b[1]) + m(a[2], b[2]) + m(a[1], b[3]) + m(a[0], b[4]);

        let mut out = [0u64; 5];
        c1 += (c0 >> 51) as u64 as u128;
        out[0] = (c0 as u64) & MASK51;
        c2 += (c1 >> 51) as u64 as u128;
        out[1] = (c1 as u64) & MASK51;
        c3 += (c2 >> 51) as u64 as u128;
        out[2] = (c2 as u64) & MASK51;
        c4 += (c3 >> 51) as u64 as u128;
        out[3] = (c3 as u64) & MASK51;
        let carry = (c4 >> 51) as u64;
        out[4] = (c4 as u64) & MASK51;
        out[0] += carry * 19;
        out[1] += out[0] >> 51;
        out[0] &= MASK51;
        Fe(out)
    }

    /// Fifteen limb products rather than twenty-five, since every cross term appears twice.
    /// Doubling is mostly squaring, and verification is mostly doubling.
    fn square(self) -> Fe {
        #[inline(always)]
        fn m(x: u64, y: u64) -> u128 {
            (x as u128) * (y as u128)
        }
        let a = self.0;
        let (a3_19, a4_19) = (a[3] * 19, a[4] * 19);
        let c0 = m(a[0], a[0]) + 2 * (m(a[1], a4_19) + m(a[2], a3_19));
        let mut c1 = m(a[3], a3_19) + 2 * (m(a[0], a[1]) + m(a[2], a4_19));
        let mut c2 = m(a[1], a[1]) + 2 * (m(a[0], a[2]) + m(a[4], a3_19));
        let mut c3 = m(a[4], a4_19) + 2 * (m(a[0], a[3]) + m(a[1], a[2]));
        let mut c4 = m(a[2], a[2]) + 2 * (m(a[0], a[4]) + m(a[1], a[3]));

        let mut out = [0u64; 5];
        c1 += (c0 >> 51) as u64 as u128;
        out[0] = (c0 as u64) & MASK51;
        c2 += (c1 >> 51) as u64 as u128;
        out[1] = (c1 as u64) & MASK51;
        c3 += (c2 >> 51) as u64 as u128;
        out[2] = (c2 as u64) & MASK51;
        c4 += (c3 >> 51) as u64 as u128;
        out[3] = (c3 as u64) & MASK51;
        let carry = (c4 >> 51) as u64;
        out[4] = (c4 as u64) & MASK51;
        out[0] += carry * 19;
        out[1] += out[0] >> 51;
        out[0] &= MASK51;
        Fe(out)
    }

    /// Square `k` times.
    fn pow2k(self, k: u32) -> Fe {
        let mut x = self;
        for _ in 0..k {
            x = x.square();
        }
        x
    }

    /// z^(2²⁵⁰ − 1) and z^11, the two pieces both exponentiations below are built from.
    ///
    /// The standard addition chain: 250 squarings and a dozen multiplications, against about
    /// five hundred operations for plain square-and-multiply.
    fn pow22501(self) -> (Fe, Fe) {
        let t0 = self.square(); // 2
        let t1 = t0.pow2k(2); // 8
        let t1 = self.mul(t1); // 9
        let t0 = t0.mul(t1); // 11
        let t2 = t0.square(); // 22
        let t1 = t1.mul(t2); // 31 = 2^5 - 1
        let t2 = t1.pow2k(5);
        let t1 = t2.mul(t1); // 2^10 - 1
        let t2 = t1.pow2k(10);
        let t2 = t2.mul(t1); // 2^20 - 1
        let t3 = t2.pow2k(20);
        let t2 = t3.mul(t2); // 2^40 - 1
        let t2 = t2.pow2k(10);
        let t1 = t2.mul(t1); // 2^50 - 1
        let t2 = t1.pow2k(50);
        let t2 = t2.mul(t1); // 2^100 - 1
        let t3 = t2.pow2k(100);
        let t2 = t3.mul(t2); // 2^200 - 1
        let t2 = t2.pow2k(50);
        let t1 = t2.mul(t1); // 2^250 - 1
        (t1, t0)
    }

    /// The multiplicative inverse, as z^(p − 2).
    fn invert(self) -> Fe {
        let (t19, t3) = self.pow22501();
        t19.pow2k(5).mul(t3)
    }

    /// z^((p − 5)/8), which is most of a square root.
    fn pow_p58(self) -> Fe {
        let (t19, _) = self.pow22501();
        t19.pow2k(2).mul(self)
    }

    fn is_zero(self) -> bool {
        self.to_bytes() == [0; 32]
    }

    /// Whether the canonical value is odd — the "sign" RFC 8032 stores in the top bit.
    fn is_negative(self) -> bool {
        self.to_bytes()[0] & 1 == 1
    }

    fn equals(self, other: Fe) -> bool {
        self.to_bytes() == other.to_bytes()
    }

    /// √(u/v) if there is one.
    ///
    /// RFC 8032 §5.1.3: a candidate is u·v³·(u·v⁷)^((p−5)/8), which is right up to a factor of
    /// √−1; multiplying by √−1 fixes the one case, and in the other the ratio has no root.
    fn sqrt_ratio(u: Fe, v: Fe) -> Option<Fe> {
        let v3 = v.square().mul(v);
        let v7 = v3.square().mul(v);
        let mut x = u.mul(v3).mul(u.mul(v7).pow_p58());
        let check = v.mul(x.square());
        if check.equals(u) {
            // x is a root.
        } else if check.equals(u.neg()) {
            x = x.mul(SQRT_M1);
        } else {
            return None;
        }
        Some(x)
    }
}

/// A point on the curve, in extended coordinates: x = X/Z, y = Y/Z, and T = XY/Z.
#[derive(Clone, Copy, Debug)]
struct Point {
    x: Fe,
    y: Fe,
    z: Fe,
    t: Fe,
}

const IDENTITY: Point = Point {
    x: Fe::ZERO,
    y: Fe::ONE,
    z: Fe::ONE,
    t: Fe::ZERO,
};

/// The base point, y = 4/5 with x even, whose multiples are every public key.
const BASE: Point = Point {
    x: Fe([
        0x62d608f25d51a,
        0x412a4b4f6592a,
        0x75b7171a4b31d,
        0x1ff60527118fe,
        0x216936d3cd6e5,
    ]),
    y: Fe([
        0x6666666666658,
        0x4cccccccccccc,
        0x1999999999999,
        0x3333333333333,
        0x6666666666666,
    ]),
    z: Fe::ONE,
    t: Fe([
        0x68ab3a5b7dda3,
        0xeea2a5eadbb,
        0x2af8df483c27e,
        0x332b375274732,
        0x67875f0fd78b7,
    ]),
};

impl Point {
    /// "add-2008-hwcd-3". Complete: it is correct for any two points, including equal ones.
    fn add(&self, q: &Point) -> Point {
        let a = self.y.sub(self.x).mul(q.y.sub(q.x));
        let b = self.y.add(self.x).mul(q.y.add(q.x));
        let c = self.t.mul(D2).mul(q.t);
        let d = self.z.add(self.z).mul(q.z);
        let e = b.sub(a);
        let f = d.sub(c);
        let g = d.add(c);
        let h = b.add(a);
        Point {
            x: e.mul(f),
            y: g.mul(h),
            t: e.mul(h),
            z: f.mul(g),
        }
    }

    /// "dbl-2008-hwcd", which is cheaper than adding a point to itself.
    fn double(&self) -> Point {
        let a = self.x.square();
        let b = self.y.square();
        let zz = self.z.square();
        let c = zz.add(zz);
        let h = a.add(b);
        let e = h.sub(self.x.add(self.y).square());
        let g = a.sub(b);
        let f = c.add(g);
        Point {
            x: e.mul(f),
            y: g.mul(h),
            t: e.mul(h),
            z: f.mul(g),
        }
    }

    fn neg(&self) -> Point {
        Point {
            x: self.x.neg(),
            y: self.y,
            z: self.z,
            t: self.t.neg(),
        }
    }

    /// y, with the sign of x in the top bit.
    fn encode(&self) -> [u8; 32] {
        let inverse = self.z.invert();
        let x = self.x.mul(inverse);
        let y = self.y.mul(inverse);
        let mut out = y.to_bytes();
        out[31] |= (x.is_negative() as u8) << 7;
        out
    }

    /// RFC 8032 §5.1.3, strictly: a y that is not below p is refused rather than reduced, so
    /// every point has exactly one encoding and a signature cannot be varied by re-encoding the
    /// key it names.
    fn decode(bytes: &[u8; 32]) -> Option<Point> {
        let sign = bytes[31] >> 7 == 1;
        let y = Fe::from_bytes(bytes);
        let mut canonical = y.to_bytes();
        canonical[31] |= bytes[31] & 0x80;
        if canonical != *bytes {
            return None;
        }
        let yy = y.square();
        let u = yy.sub(Fe::ONE);
        let v = D.mul(yy).add(Fe::ONE);
        let mut x = Fe::sqrt_ratio(u, v)?;
        if x.is_zero() && sign {
            return None;
        }
        if x.is_negative() != sign {
            x = x.neg();
        }
        Some(Point {
            x,
            y,
            z: Fe::ONE,
            t: x.mul(y),
        })
    }

    /// [scalar]·self, four bits at a time.
    fn mul(&self, scalar: &[u8; 32]) -> Point {
        let mut table = [IDENTITY; 16];
        table[1] = *self;
        for i in 2..16 {
            table[i] = table[i - 1].add(self);
        }
        let mut acc = IDENTITY;
        for byte in scalar.iter().rev() {
            for nibble in [byte >> 4, byte & 15] {
                acc = acc.double().double().double().double();
                if nibble != 0 {
                    acc = acc.add(&table[nibble as usize]);
                }
            }
        }
        acc
    }

    /// [scalar]·B, from a table of every multiple of every power of sixteen of the base point.
    ///
    /// Sixty-four additions and no doublings. The table is built once per process, the first
    /// time anything signs or verifies, and costs about a millisecond.
    fn mul_base(scalar: &[u8; 32]) -> Point {
        let table = base_table();
        let mut acc = IDENTITY;
        for (i, byte) in scalar.iter().enumerate() {
            let (low, high) = ((byte & 15) as usize, (byte >> 4) as usize);
            if low != 0 {
                acc = acc.add(&table[2 * i][low]);
            }
            if high != 0 {
                acc = acc.add(&table[2 * i + 1][high]);
            }
        }
        acc
    }
}

fn base_table() -> &'static [[Point; 16]] {
    static TABLE: OnceLock<Vec<[Point; 16]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = Vec::with_capacity(64);
        let mut power = BASE;
        for _ in 0..64 {
            let mut row = [IDENTITY; 16];
            for j in 1..16 {
                row[j] = row[j - 1].add(&power);
            }
            table.push(row);
            power = power.double().double().double().double();
        }
        table
    })
}

/// The order of the base point, as four little-endian 64-bit limbs.
const L: [u64; 4] = [0x5812631a5cf5d3ed, 0x14def9dea2f79cd6, 0, 0x1000000000000000];

/// A 512-bit little-endian number, reduced modulo L.
///
/// Restoring division one bit at a time, which is about the slowest correct way to do it and
/// is still a few microseconds. It runs twice per signature and once per verification.
fn reduce_wide(x: &[u64; 8]) -> [u64; 4] {
    let mut r = [0u64; 5];
    for bit in (0..512).rev() {
        // r = 2r + bit. r < L before, so 2r + 1 < 2L < 2²⁵⁴, which fits.
        let mut carry = (x[bit / 64] >> (bit % 64)) & 1;
        for limb in &mut r {
            let next = *limb >> 63;
            *limb = (*limb << 1) | carry;
            carry = next;
        }
        if !below_l(&r) {
            let mut borrow = 0u64;
            for i in 0..5 {
                let l = if i < 4 { L[i] } else { 0 };
                let (d1, b1) = r[i].overflowing_sub(l);
                let (d2, b2) = d1.overflowing_sub(borrow);
                r[i] = d2;
                borrow = (b1 | b2) as u64;
            }
        }
    }
    [r[0], r[1], r[2], r[3]]
}

fn below_l(r: &[u64; 5]) -> bool {
    if r[4] != 0 {
        return false;
    }
    for i in (0..4).rev() {
        if r[i] != L[i] {
            return r[i] < L[i];
        }
    }
    false
}

fn limbs(bytes: &[u8]) -> Vec<u64> {
    bytes
        .chunks(8)
        .map(|c| {
            let mut word = [0u8; 8];
            word[..c.len()].copy_from_slice(c);
            u64::from_le_bytes(word)
        })
        .collect()
}

fn scalar_bytes(limbs: [u64; 4]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (chunk, limb) in out.chunks_exact_mut(8).zip(limbs) {
        chunk.copy_from_slice(&limb.to_le_bytes());
    }
    out
}

/// A 64-byte hash output, read as a number and reduced modulo L.
fn scalar_from_hash(hash: &[u8; 64]) -> [u8; 32] {
    let words = limbs(hash);
    let wide: [u64; 8] = words.try_into().expect("eight limbs");
    scalar_bytes(reduce_wide(&wide))
}

/// (a·b + c) mod L, for a, b, c below 2²⁵⁶.
fn mul_add(a: &[u8; 32], b: &[u8; 32], c: &[u8; 32]) -> [u8; 32] {
    let (a, b, c) = (limbs(a), limbs(b), limbs(c));
    let mut wide = [0u128; 9];
    for i in 0..4 {
        for j in 0..4 {
            let product = (a[i] as u128) * (b[j] as u128);
            wide[i + j] += product & u64::MAX as u128;
            wide[i + j + 1] += product >> 64;
        }
    }
    for (i, limb) in c.iter().enumerate() {
        wide[i] += *limb as u128;
    }
    let mut out = [0u64; 8];
    let mut carry = 0u128;
    for i in 0..8 {
        let v = wide[i] + carry;
        out[i] = v as u64;
        carry = v >> 64;
    }
    debug_assert_eq!(carry + wide[8], 0, "a product of two 256-bit numbers fits in 512 bits");
    scalar_bytes(reduce_wide(&out))
}

/// Whether a 32-byte scalar is already reduced — RFC 8032 requires S < L, and without the
/// check anybody can turn one valid signature into a second by adding L to S.
fn is_canonical_scalar(s: &[u8; 32]) -> bool {
    let words = limbs(s);
    let r = [words[0], words[1], words[2], words[3], 0];
    below_l(&r)
}

/// Somebody's ability to sign: 32 secret bytes, and what RFC 8032 derives from them.
#[derive(Clone)]
pub struct SigningKey {
    seed: [u8; 32],
    scalar: [u8; 32],
    prefix: [u8; 32],
    public: PublicKey,
}

impl std::fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The secret is left out of debug output on principle, even here.
        write!(f, "SigningKey({})", self.public)
    }
}

impl SigningKey {
    /// The key for a 32-byte seed.
    ///
    /// Everything is derived, so the seed *is* the key: the same 32 bytes always sign the
    /// same way, which is how a world founded from one seed can sign its whole history again
    /// and get the same chain.
    pub fn from_seed(seed: [u8; 32]) -> SigningKey {
        let mut hasher = Sha512::new();
        hasher.update(&seed);
        let h = hasher.finish();
        let mut scalar: [u8; 32] = h[..32].try_into().expect("32 bytes");
        // Clamped: a multiple of eight, so it kills the small-order part of any point, and
        // with the top bit fixed, so the ladder always has the same length.
        scalar[0] &= 248;
        scalar[31] &= 127;
        scalar[31] |= 64;
        let prefix: [u8; 32] = h[32..].try_into().expect("32 bytes");
        let public = PublicKey(Point::mul_base(&scalar).encode());
        SigningKey {
            seed,
            scalar,
            prefix,
            public,
        }
    }

    pub fn seed(&self) -> [u8; 32] {
        self.seed
    }

    pub fn public(&self) -> PublicKey {
        self.public
    }

    /// RFC 8032 §5.1.6.
    pub fn sign(&self, message: &[u8]) -> Signature {
        let mut nonce = Sha512::new();
        nonce.update(&self.prefix);
        nonce.update(message);
        let r = scalar_from_hash(&nonce.finish());
        let big_r = Point::mul_base(&r).encode();

        let mut challenge = Sha512::new();
        challenge.update(&big_r);
        challenge.update(&self.public.0);
        challenge.update(message);
        let k = scalar_from_hash(&challenge.finish());

        let s = mul_add(&k, &self.scalar, &r);
        let mut out = [0u8; 64];
        out[..32].copy_from_slice(&big_r);
        out[32..].copy_from_slice(&s);
        Signature(out)
    }
}

/// A public key: the encoding of a point.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicKey(pub [u8; 32]);

impl std::fmt::Display for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", crate::hex(&self.0))
    }
}

impl std::fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PublicKey({}…)", &crate::hex(&self.0)[..12])
    }
}

impl PublicKey {
    /// RFC 8032 §5.1.7, cofactorless: [S]B = R + [k]A, checked by computing [S]B − [k]A and
    /// comparing its encoding with R's.
    ///
    /// Refuses an S that is not reduced, and a key or an R that is not canonically encoded, so
    /// there is exactly one valid signature per (key, message) pair a signer would produce —
    /// the property a ledger needs if a transaction's identity is its hash.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> bool {
        let big_r: [u8; 32] = signature.0[..32].try_into().expect("32 bytes");
        let s: [u8; 32] = signature.0[32..].try_into().expect("32 bytes");
        if !is_canonical_scalar(&s) {
            return false;
        }
        let Some(a) = Point::decode(&self.0) else {
            return false;
        };
        let mut challenge = Sha512::new();
        challenge.update(&big_r);
        challenge.update(&self.0);
        challenge.update(message);
        let k = scalar_from_hash(&challenge.finish());
        let check = Point::mul_base(&s).add(&a.neg().mul(&k));
        check.encode() == big_r
    }

    /// Whether these bytes are a point at all.
    pub fn is_valid(&self) -> bool {
        Point::decode(&self.0).is_some()
    }
}

/// A signature: R, then S.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Signature(pub [u8; 64]);

impl std::fmt::Debug for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Signature({}…)", &crate::hex(&self.0)[..12])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hex, unhex};

    fn key(seed: &str) -> SigningKey {
        SigningKey::from_seed(unhex(seed).try_into().expect("32-byte seed"))
    }

    /// RFC 8032 §7.1, tests 1, 2, 3 and SHA(abc) — which OpenSSL 3.0 also reproduces.
    #[test]
    fn the_rfc_vectors_sign_and_verify() {
        let vectors = [
            (
                "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
                "",
                "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
                "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
            ),
            (
                "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
                "72",
                "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
                "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
            ),
            (
                "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
                "af82",
                "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
                "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
            ),
            (
                "833fe62409237b9d62ec77587520911e9a759cec1d19755b7da901b96dca3d42",
                "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
                "ec172b93ad5e563bf4932c70e1245034c35467ef2efd4d64ebf819683467e2bf",
                "dc2a4459e7369633a52b1bf277839a00201009a3efbf3ecb69bea2186c26b58909351fc9ac90b3ecfdfbc7c66431e0303dca179c138ac17ad9bef1177331a704",
            ),
        ];
        for (seed, message, public, signature) in vectors {
            let key = key(seed);
            let message = unhex(message);
            assert_eq!(hex(&key.public().0), public, "public key for {seed}");
            let signed = key.sign(&message);
            assert_eq!(hex(&signed.0), signature, "signature for {seed}");
            assert!(key.public().verify(&message, &signed));
        }
    }

    /// Eight more, of assorted lengths, produced by `openssl pkeyutl -sign -rawin`. An
    /// implementation that agreed with the RFC's four and disagreed here would be one that
    /// had got lucky on short messages.
    #[test]
    fn it_agrees_with_openssl() {
        let cases = [
            (
                "25a1446f3ae0b13ec802e5a70f5b130ded59a4d940e9eb089c5942910a270ab4",
                "00",
                "474671fb6d7ed3f20cd98d7d142c5db8dd076fa6a06eec0703ed1c6607325bd9",
                "4ed8d339669aea8716d63a60decf3333424e5fe88660d24799d012184b8169199e9f56234090f14dea0082f1ff50fd7a5192fee0c249f7d66a3523a426bec50e",
            ),
            (
                "b5a82b8d2cd69699866482d573b7d438378baf7b5c2de39192e21b541e4a63e6",
                "3d18456b3800a9cf4f8a22b0d25c61ae7bee4c6d07683829d9f8ddf2ed9c6e8539de1acc94a477335c3f00597620889b0924dba1f747d0bf9ac5ba812da56035000102030405060708090a0b0c0d",
                "d32c06878e8e58a08bc6307922d992c35672d6fd47d9c17fa2cf0a975c17247f",
                "71c0670d10448787c3314b43299cdf20b2c218b8737ed164339a0af865214352259e5462739864cf3b277a986e4ebf638c8cff7cb1c1fc644a1d472796899d00",
            ),
            (
                "3b5f1da66bd86f400d0ed0ac55e282ce782d98c3dc2f40ceb9c054154344cf09",
                "0ee62dc20b9cfff7c616735cee99ff1d73a111dd99cbe294a59c7024addb9a50dd6009e0e66c48b399302f49334eefe004f39700bf9a281054c0b57f54323b4b0ee62dc20b9cfff7c616735cee99ff1d73a111dd99cbe294a59c7024addb9a50dd6009e0e66c48b399302f49334eefe004f39700bf9a281054c0b57f54323b4b000102030405060708090a0b0c0d0e0f101112131415161718191a",
                "1db91b5f395cc4ba6dfffb510bf324f7f8703f72611a911448bb808c7457b836",
                "91300fedc646640df58b0d3df033bc1faa8113d9547a21739fbca953fd9fba5c43cf80b586398330f2530c10ac4330b3870ad9dc5d3d8693e209463be7f5e900",
            ),
            (
                "6fe7ee476a2bdbb3574b914aba45dfb95280fa0d14cefbffc842a65a2c5db10f",
                "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f2021222324252627",
                "868f6b498968c1de6275149fdce534d37dec9ed3881994ad121cb4047f34c00d",
                "737e29e1cf84db13f9fd3738e4037a211c24c3240906d7ab0e8bf1a22c3abd0e3afc4b920ee97efb51db18e1470ef915e67f90783f6d5e1f56de923b3fe5f403",
            ),
            (
                "96126c9890673a384b1670cd07941aa939ab28a2672c6ba1e9e1371f2ef43971",
                "03c6b97a3506956dedd61b6b4b887c3f4fb6ed0853ecfe2d375e8e69ec6f2945029c394b001c07d743e01297811743c0863065ba6850791aea310dcfde58a718000102",
                "4c4e1e99b263bd541e9995abf751561f84e4675b12f54b22a15690069ef9191f",
                "ad0eb02c12150af4970168193abe403ddfd366d0c8442223e302058bd8a5d3a7d7ebe463301141ca0d661a4c300c704c24f5a145dfecd11837c9978db003fe04",
            ),
            (
                "ec706b74749253f111d44696b4c0a21de44eac829645c89608ecd727f74508e9",
                "400e69814c37c9f1a22774e53a20efc22896d6e9f65c687ae924c065bc30ebfaf2c90d23b41702ab36681ca96698364e833acd408d2907cda5a3cc3b5804f118400e69814c37c9f1a22774e53a20efc22896d6e9f65c687ae924c065bc30ebfaf2c90d23b41702ab36681ca96698364e833acd408d2907cda5a3cc3b5804f118000102030405060708090a0b0c0d0e0f",
                "0cd0e3b35930366eec155606ba30c4b949b8ae888def8d79aa89e961479ac5db",
                "74da552feaf6e21bfeefe4bab65a67535860761a107fff40db44a8f5d7dfa0592e46781527e9510f760e50bc595a8a491d6033949bd2dc1ee99528f53464fe08",
            ),
            (
                "075e5ea5d25fd4986c05c3188d52d4779ddebb40753d4e138bb2fa464f587042",
                "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c",
                "6e288c178295823dd2859996d46e5fd8507639a5864cdb9f70877574a7fbbc29",
                "da843c94ce96c162f805f8853d83b27296a022a96f9b8e6f4269f96a896105038b6237fbec8cb8d366ee0ab4950d90b3f070a9fe84d085bc8c654cf8a9032708",
            ),
            (
                "560dcd05bfe7ac63ea5734a62a7c0289dd59638145e87446ecf2b1125debdf30",
                "8e21a984198be91d4971664fe2fb5d4b4eb5e0fd7f0d53316942a2e413fb28a99f814771c565bfe09e179794dc6795f5ae7010959e1c764cf79f3cf31e8890b8000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20212223242526272829",
                "d7e170db2d1a64b85078b838ba21dcd72cc003c564a3aefc8749e616e5460523",
                "cad9823b88c8cacc2e125631d704bb3e02396009e95265b2aa8c827e0b7b14b1d0c24e146dec892c35d23e57b6906ec2e422fe628a36c28b2239ddc8e5ea2401",
            ),
        ];
        for (seed, message, public, signature) in cases {
            let key = key(seed);
            let message = unhex(message);
            assert_eq!(hex(&key.public().0), public);
            assert_eq!(hex(&key.sign(&message).0), signature);
        }
    }

    #[test]
    fn a_signature_is_for_one_message_and_one_key() {
        let alice = key("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
        let bob = key("4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb");
        let signed = alice.sign(b"pay bob 10");
        assert!(alice.public().verify(b"pay bob 10", &signed));
        assert!(!alice.public().verify(b"pay bob 11", &signed), "another message");
        assert!(!bob.public().verify(b"pay bob 10", &signed), "another key");
        for byte in [0, 17, 31, 32, 50, 63] {
            let mut forged = signed;
            forged.0[byte] ^= 1;
            assert!(
                !alice.public().verify(b"pay bob 10", &forged),
                "a signature with byte {byte} changed still verified"
            );
        }
    }

    /// S + L is the same scalar and a different byte string. Accepting it would give every
    /// signed transaction a second, equally valid encoding — and a second id.
    #[test]
    fn an_unreduced_s_is_refused() {
        let alice = key("c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7");
        let signed = alice.sign(b"once");
        let s: [u8; 32] = signed.0[32..].try_into().unwrap();
        let bumped = {
            let (s, l) = (limbs(&s), L);
            let mut out = [0u64; 4];
            let mut carry = 0u128;
            for i in 0..4 {
                let v = s[i] as u128 + l[i] as u128 + carry;
                out[i] = v as u64;
                carry = v >> 64;
            }
            assert_eq!(carry, 0, "S + L still fits in 256 bits");
            scalar_bytes(out)
        };
        let mut malleated = signed;
        malleated.0[32..].copy_from_slice(&bumped);
        assert_ne!(malleated, signed);
        assert!(!alice.public().verify(b"once", &malleated));
    }

    /// Anything that is not a point is not a key.
    #[test]
    fn a_key_that_is_not_a_point_verifies_nothing() {
        // y = 2 has no x on this curve.
        let mut not_a_point = [0u8; 32];
        not_a_point[0] = 2;
        assert!(!PublicKey(not_a_point).is_valid());
        let alice = key("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
        assert!(alice.public().is_valid());
        let signed = alice.sign(b"x");
        assert!(!PublicKey(not_a_point).verify(b"x", &signed));
        // And y = p, which is 0 written the long way, is refused rather than reduced.
        let mut long_zero = [0xffu8; 32];
        long_zero[0] = 0xed;
        long_zero[31] = 0x7f;
        assert!(!PublicKey(long_zero).is_valid());
    }

    #[test]
    fn field_arithmetic_round_trips() {
        let x = Fe::from_bytes(&[7u8; 32]);
        assert!(x.mul(x.invert()).equals(Fe::ONE));
        assert!(x.sub(x).is_zero());
        assert!(x.add(x.neg()).is_zero());
        assert!(SQRT_M1.square().equals(Fe::ONE.neg()));
        // The base point is on the curve: −x² + y² = 1 + d·x²·y².
        let (xx, yy) = (BASE.x.square(), BASE.y.square());
        assert!(yy.sub(xx).equals(Fe::ONE.add(D.mul(xx).mul(yy))));
        // And its encoding is the one everybody knows.
        assert_eq!(
            hex(&BASE.encode()),
            "5866666666666666666666666666666666666666666666666666666666666666"
        );
        // [L]B is the identity, which is what "the order of B" means.
        let l_bytes = scalar_bytes(L);
        assert_eq!(Point::mul_base(&l_bytes).encode(), IDENTITY.encode());
        assert_eq!(BASE.mul(&l_bytes).encode(), IDENTITY.encode());
    }

    #[test]
    fn the_two_ways_of_multiplying_agree() {
        for seed in 0u8..6 {
            let scalar = crate::sha2::sha256(&[seed; 5]);
            assert_eq!(
                Point::mul_base(&scalar).encode(),
                BASE.mul(&scalar).encode(),
                "fixed-base and variable-base multiplication disagree"
            );
        }
    }
}

#[cfg(test)]
mod measure {
    use super::*;

    /// How long signing and verifying take, which sets how many signatures a simulated year
    /// can afford. A measurement, not an assertion.
    #[test]
    #[ignore]
    fn measure_signing() {
        let key = SigningKey::from_seed([9; 32]);
        let message = [7u8; 120];
        let _ = key.sign(&message); // build the base table outside the timing
        let n: u32 = 2000;
        let start = std::time::Instant::now();
        for i in 0..n {
            let _ = key.sign(&[message.as_slice(), &i.to_le_bytes()].concat());
        }
        let signing = start.elapsed() / n;
        let last = key.sign(&message);
        let start = std::time::Instant::now();
        let mut ok = 0;
        for _ in 0..n {
            ok += key.public().verify(&message, &last) as u32;
        }
        let verifying = start.elapsed() / n;
        let start = std::time::Instant::now();
        for i in 0..n {
            let _ = crate::sha2::sha256(&[message.as_slice(), &i.to_le_bytes()].concat());
        }
        let hashing = start.elapsed() / n;
        eprintln!("sign {signing:?}  verify {verifying:?}  sha256(124 B) {hashing:?}  ({ok})");
    }
}
