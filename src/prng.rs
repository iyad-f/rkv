// SPDX-FileCopyrightText: 2026 Iyad
// SPDX-License-Identifier: Apache-2.0

//! A simple xorshift pseudo-random number generator.
//!
//! See <https://en.wikipedia.org/wiki/Xorshift>.

/// An xorshift pseudo-random number generator with 64-bit state.
pub struct Prng {
    /// The generator's current state, never zero.
    state: u64,
}

impl Prng {
    /// Creates a generator seeded with `seed`. A zero seed is replaced with a
    /// fixed nonzero constant, since a zero state would make xorshift emit only
    /// zeros forever.
    pub fn new(seed: u64) -> Self {
        // The fallback is the golden ratio constant. This just skips the warm up
        // period which would be required if a simple seed like 1 or 2 was used, since
        // those mostly have unset bits in them and xorshift needs a few rounds to spread
        // the set bits across all 64 before its output looks random. The golden
        // ratio's bits are already well spread out, so there's no warm up. Its exact
        // value isn't special though, any nonzero value with well mixed bits works, I
        // just got to see this being used mostly when i searched so i took it.
        let state = if seed == 0 { 0x9E3779B97F4A7C15 } else { seed };
        Self { state }
    }

    /// Advances the generator and returns the next pseudo-random number.
    pub fn next_rand(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }
}
