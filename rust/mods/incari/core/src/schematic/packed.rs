//! Small numbers packed tightly into 64-bit words, `bits` each, low bits first; an entry may run
//! on into the next word. A `.litematic` keeps a region's blocks this way, so its array is read as
//! it is, and every format's blocks are kept so: a few bits a block, not a word.

#[derive(Clone, Debug, PartialEq)]
pub struct Packed {
    bits: u32,
    mask: u64,
    len: usize,
    data: Vec<u64>,
}

/// How many words `len` entries of `bits` take.
pub fn words_for(bits: u32, len: usize) -> usize {
    (bits as usize * len).div_ceil(64)
}

/// The fewest bits that hold `0` to `count - 1`; at least `min` (and 1).
pub fn bits_for(count: usize, min: u32) -> u32 {
    let bits = usize::BITS - count.saturating_sub(1).leading_zeros();
    bits.max(min).max(1)
}

impl Packed {
    /// `len` zeros of `bits` bits each (1 to 32).
    pub fn zeros(bits: u32, len: usize) -> Self {
        Self::over(bits, len, vec![0; words_for(bits, len)]).expect("sized to fit")
    }

    /// Over `data` as it is; None if the packing is bad or `data` is too short.
    pub fn over(bits: u32, len: usize, data: Vec<u64>) -> Option<Self> {
        if !(1..=32).contains(&bits) || data.len() < words_for(bits, len) {
            return None;
        }
        Some(Self { bits, mask: (1u64 << bits) - 1, len, data })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn bits(&self) -> u32 {
        self.bits
    }

    /// Entry `index`. Hot path: arithmetic only.
    #[inline]
    pub fn get(&self, index: usize) -> u32 {
        let bit = index * self.bits as usize;
        let word = bit >> 6;
        let offset = (bit & 63) as u32;
        let mut value = self.data[word] >> offset;
        if offset + self.bits > 64 {
            value |= self.data[word + 1] << (64 - offset);
        }
        (value & self.mask) as u32
    }

    /// Sets entry `index` to `value`, which must fit in `bits`.
    pub fn set(&mut self, index: usize, value: u32) {
        let bit = index * self.bits as usize;
        let word = bit >> 6;
        let offset = (bit & 63) as u32;
        let v = u64::from(value) & self.mask;
        self.data[word] = (self.data[word] & !(self.mask << offset)) | (v << offset);
        if offset + self.bits > 64 {
            let spill = 64 - offset;
            self.data[word + 1] = (self.data[word + 1] & !(self.mask >> spill)) | (v >> spill);
        }
    }

    /// A copy with `bits` bits an entry, for values that outgrew these.
    pub fn widened(&self, bits: u32) -> Self {
        let mut wider = Self::zeros(bits, self.len);
        for i in 0..self.len {
            wider.set(i, self.get(i));
        }
        wider
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_span_words() {
        let mut p = Packed::zeros(5, 40);
        for i in 0..40 {
            p.set(i, (i as u32 * 7) & 31);
        }
        for i in 0..40 {
            assert_eq!(p.get(i), (i as u32 * 7) & 31, "{i}");
        }
        let w = p.widened(9);
        assert_eq!((0..40).map(|i| w.get(i)).collect::<Vec<_>>(), (0..40).map(|i| p.get(i)).collect::<Vec<_>>());
    }

    #[test]
    fn bits_for_counts() {
        assert_eq!(bits_for(1, 2), 2);
        assert_eq!(bits_for(4, 2), 2);
        assert_eq!(bits_for(5, 2), 3);
        assert_eq!(bits_for(256, 1), 8);
        assert_eq!(bits_for(257, 1), 9);
        assert_eq!(bits_for(0, 0), 1);
        assert!(Packed::over(5, 40, vec![0; 3]).is_none());
        assert!(Packed::over(5, 40, vec![0; 4]).is_some());
    }
}
