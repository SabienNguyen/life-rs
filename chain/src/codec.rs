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

#[cfg(test)]
mod tests {
    use super::*;

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
