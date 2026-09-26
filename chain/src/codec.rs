//! One way of writing anything down, so that everybody hashes the same bytes.
//!
//! A signature is over bytes, not over a meaning. If two honest nodes could encode the same
//! transaction two ways, a signature over one would not verify against the other and the
//! chain would split over a formatting difference. So there is exactly one encoding: fixed
//! little-endian integers, fixed-width keys and hashes, a length before anything whose length
//! varies, and a **domain tag** at the front of every kind of thing that gets signed or hashed.
//!
//! The tag is what stops a signature made for one purpose being replayed as another. A vote
//! and a transaction are both "some bytes signed by a validator", and without the tag a
//! carefully chosen transaction could be made to encode to the same bytes as a vote.

/// A canonical encoder.
pub struct Writer(Vec<u8>);

impl Writer {
    /// Start an encoding for one kind of thing.
    pub fn tagged(domain: &str) -> Writer {
        let mut writer = Writer(Vec::with_capacity(128));
        writer.text(domain);
        writer
    }

    pub fn u8(&mut self, value: u8) -> &mut Writer {
        self.0.push(value);
        self
    }

    pub fn u32(&mut self, value: u32) -> &mut Writer {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn u64(&mut self, value: u64) -> &mut Writer {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn u128(&mut self, value: u128) -> &mut Writer {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn i64(&mut self, value: i64) -> &mut Writer {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    /// Bytes whose length is fixed by what they are — a key, a hash, a signature.
    pub fn fixed(&mut self, bytes: &[u8]) -> &mut Writer {
        self.0.extend_from_slice(bytes);
        self
    }

    /// Bytes whose length is not, preceded by it.
    pub fn var(&mut self, bytes: &[u8]) -> &mut Writer {
        self.u32(bytes.len() as u32);
        self.0.extend_from_slice(bytes);
        self
    }

    pub fn text(&mut self, text: &str) -> &mut Writer {
        self.var(text.as_bytes())
    }

    pub fn finish(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.0)
    }
}

/// Why some bytes are not the encoding of anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Malformed {
    /// They end before the thing does.
    Short,
    /// They are the encoding of some other kind of thing.
    WrongTag,
    /// The thing ends before they do.
    Trailing,
    /// A field holds a value no encoder writes: an unknown kind, text that is not UTF-8, a
    /// count too large to be real.
    BadValue(&'static str),
}

/// The only decoder there is for what `Writer` writes, and exactly as strict: fixed-width
/// little-endian integers, a length before anything that varies, the tag first, and nothing
/// after the end. Whatever it accepts, `Writer` writes back to the same bytes — which is what
/// makes a chain read from a file the chain that was written, and not merely one like it.
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    /// Start reading one kind of thing, which must be the one the bytes say they are.
    pub fn tagged(bytes: &'a [u8], domain: &str) -> Result<Reader<'a>, Malformed> {
        let mut reader = Reader { bytes, at: 0 };
        let tag = reader.var().map_err(|_| Malformed::WrongTag)?;
        if tag != domain.as_bytes() {
            return Err(Malformed::WrongTag);
        }
        Ok(reader)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        let end = self.at.checked_add(n).ok_or(Malformed::Short)?;
        let taken = self.bytes.get(self.at..end).ok_or(Malformed::Short)?;
        self.at = end;
        Ok(taken)
    }

    pub fn u8(&mut self) -> Result<u8, Malformed> {
        Ok(self.take(1)?[0])
    }

    pub fn u32(&mut self) -> Result<u32, Malformed> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    pub fn u64(&mut self) -> Result<u64, Malformed> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    pub fn u128(&mut self) -> Result<u128, Malformed> {
        Ok(u128::from_le_bytes(self.fixed()?))
    }

    pub fn i64(&mut self) -> Result<i64, Malformed> {
        Ok(i64::from_le_bytes(self.fixed()?))
    }

    /// Bytes whose length is fixed by what they are.
    pub fn fixed<const N: usize>(&mut self) -> Result<[u8; N], Malformed> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    /// Bytes whose length is not, preceded by it.
    pub fn var(&mut self) -> Result<&'a [u8], Malformed> {
        let n = self.u32()? as usize;
        self.take(n)
    }

    pub fn text(&mut self) -> Result<String, Malformed> {
        let bytes = self.var()?;
        String::from_utf8(bytes.to_vec()).map_err(|_| Malformed::BadValue("text that is not UTF-8"))
    }

    /// A count of things to come, each of which takes at least `each` bytes: refused at once if
    /// the bytes left could not hold that many, so a forged count cannot ask for memory the
    /// thing it describes could never fill.
    pub fn count(&mut self, each: usize) -> Result<usize, Malformed> {
        let n = self.u32()? as usize;
        if n.saturating_mul(each.max(1)) > self.bytes.len() - self.at {
            return Err(Malformed::Short);
        }
        Ok(n)
    }

    /// The end: there must be nothing after it.
    pub fn done(self) -> Result<(), Malformed> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(Malformed::Trailing)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the writer writes, the reader reads, and nothing either side of it.
    #[test]
    fn the_reader_reads_exactly_what_the_writer_wrote() {
        let bytes = Writer::tagged("t")
            .u8(7)
            .u32(8)
            .u64(9)
            .u128(10)
            .i64(-11)
            .fixed(&[1, 2, 3])
            .text("ab")
            .finish();
        let read = |bytes: &[u8]| -> Result<(), Malformed> {
            let mut r = Reader::tagged(bytes, "t")?;
            assert_eq!(r.u8()?, 7);
            assert_eq!(r.u32()?, 8);
            assert_eq!(r.u64()?, 9);
            assert_eq!(r.u128()?, 10);
            assert_eq!(r.i64()?, -11);
            assert_eq!(r.fixed::<3>()?, [1, 2, 3]);
            assert_eq!(r.text()?, "ab");
            r.done()
        };
        assert_eq!(read(&bytes), Ok(()));
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(read(&longer), Err(Malformed::Trailing));
        assert_eq!(read(&bytes[..bytes.len() - 1]), Err(Malformed::Short));
        assert_eq!(Reader::tagged(&bytes, "u").err(), Some(Malformed::WrongTag));
    }

    #[test]
    fn the_tag_keeps_two_kinds_of_thing_apart() {
        let a = Writer::tagged("vote").u64(7).finish();
        let b = Writer::tagged("pay").u64(7).finish();
        assert_ne!(a, b);
    }

    /// Without the length prefix, ("ab", "c") and ("a", "bc") would be the same bytes.
    #[test]
    fn variable_parts_cannot_slide_into_each_other() {
        let a = Writer::tagged("t").text("ab").text("c").finish();
        let b = Writer::tagged("t").text("a").text("bc").finish();
        assert_ne!(a, b);
    }
}
