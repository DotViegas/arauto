//! A QR code for the terminal of the machine of the hub: what the admin scans with the
//! telephone to pair a number.
//!
//! The encoding of ISO/IEC 18004, the known way: byte mode, error correction level M,
//! Reed-Solomon over GF(256) with the polynomial 0x11D, the blocks interleaved, the function
//! patterns, the format and version information with their BCH codes, and the mask with the
//! lowest penalty of the four rules. No library of the workspace draws QR codes, and the value
//! is a short-lived secret that must not go through anything else.

use std::fmt::Write as _;

/// The error correction codewords per block, level M, by version (index 0 unused).
const ECC_PER_BLOCK: [usize; 41] = [
    0, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28,
    28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
];

/// The number of blocks, level M, by version (index 0 unused).
const BLOCKS: [usize; 41] = [
    0, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23,
    25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49,
];

/// The two bits of level M in the format information.
const LEVEL_M_BITS: u32 = 0;

/// The text is too long for a QR code of level M.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the value is too long for a QR code")]
pub struct TooLong;

/// A QR code: a square of modules, `true` for dark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrCode {
    version: usize,
    size: usize,
    modules: Vec<bool>,
    function: Vec<bool>,
}

fn raw_data_modules(version: usize) -> usize {
    let mut result = (16 * version + 128) * version + 64;
    if version >= 2 {
        let aligns = version / 7 + 2;
        result -= (25 * aligns - 10) * aligns - 55;
        if version >= 7 {
            result -= 36;
        }
    }
    result
}

fn data_codewords(version: usize) -> usize {
    raw_data_modules(version) / 8 - ECC_PER_BLOCK[version] * BLOCKS[version]
}

fn alignment_positions(version: usize, size: usize) -> Vec<usize> {
    if version == 1 {
        return Vec::new();
    }
    let aligns = version / 7 + 2;
    let step = (version * 8 + aligns * 3 + 5) / (aligns * 4 - 4) * 2;
    let mut positions = vec![6];
    let mut position = size - 7;
    let mut rest = Vec::new();
    for _ in 0..aligns - 1 {
        rest.push(position);
        position = position.saturating_sub(step);
    }
    rest.reverse();
    positions.extend(rest);
    positions
}

fn gf_multiply(x: u8, y: u8) -> u8 {
    let mut z: u16 = 0;
    for i in (0..8).rev() {
        z = (z << 1) ^ ((z >> 7) * 0x11D);
        z ^= u16::from((y >> i) & 1) * u16::from(x);
    }
    (z & 0xFF) as u8
}

fn rs_divisor(degree: usize) -> Vec<u8> {
    let mut result = vec![0u8; degree];
    if let Some(last) = result.last_mut() {
        *last = 1;
    }
    let mut root: u8 = 1;
    for _ in 0..degree {
        for j in 0..degree {
            result[j] = gf_multiply(result[j], root);
            if j + 1 < degree {
                result[j] ^= result[j + 1];
            }
        }
        root = gf_multiply(root, 0x02);
    }
    result
}

fn rs_remainder(data: &[u8], divisor: &[u8]) -> Vec<u8> {
    let mut result = vec![0u8; divisor.len()];
    for byte in data {
        let factor = byte ^ result.first().copied().unwrap_or(0);
        result.remove(0);
        result.push(0);
        for (slot, coefficient) in result.iter_mut().zip(divisor) {
            *slot ^= gf_multiply(*coefficient, factor);
        }
    }
    result
}

impl QrCode {
    /// Encodes `text` in byte mode, level M, in the smallest version that holds it.
    ///
    /// # Errors
    /// [`TooLong`] when no version holds it.
    pub fn encode(text: &[u8]) -> Result<Self, TooLong> {
        let (version, count_bits) = (1..=40)
            .map(|version| (version, if version <= 9 { 8 } else { 16 }))
            .find(|(version, count_bits)| {
                4 + count_bits + text.len() * 8 <= data_codewords(*version) * 8
            })
            .ok_or(TooLong)?;
        let capacity = data_codewords(version) * 8;
        let mut bits: Vec<bool> = Vec::with_capacity(capacity);
        let push = |value: u32, length: usize, bits: &mut Vec<bool>| {
            for i in (0..length).rev() {
                bits.push((value >> i) & 1 == 1);
            }
        };
        push(0b0100, 4, &mut bits);
        push(
            u32::try_from(text.len()).map_err(|_| TooLong)?,
            count_bits,
            &mut bits,
        );
        for byte in text {
            push(u32::from(*byte), 8, &mut bits);
        }
        let terminator = (capacity - bits.len()).min(4);
        push(0, terminator, &mut bits);
        let padding = (8 - bits.len() % 8) % 8;
        push(0, padding, &mut bits);
        let mut data: Vec<u8> = bits
            .chunks(8)
            .map(|chunk| {
                chunk
                    .iter()
                    .fold(0u8, |byte, bit| (byte << 1) | u8::from(*bit))
            })
            .collect();
        for pad in [0xECu8, 0x11].iter().cycle() {
            if data.len() >= data_codewords(version) {
                break;
            }
            data.push(*pad);
        }
        let codewords = Self::with_ecc(version, &data);
        let size = version * 4 + 17;
        let mut code = Self {
            version,
            size,
            modules: vec![false; size * size],
            function: vec![false; size * size],
        };
        code.draw_function_patterns();
        code.draw_codewords(&codewords);
        let mut best = (u32::MAX, 0u8);
        for mask in 0..8u8 {
            code.apply_mask(mask);
            code.draw_format(mask);
            let penalty = code.penalty();
            if penalty < best.0 {
                best = (penalty, mask);
            }
            code.apply_mask(mask);
        }
        code.apply_mask(best.1);
        code.draw_format(best.1);
        Ok(code)
    }

    fn with_ecc(version: usize, data: &[u8]) -> Vec<u8> {
        let blocks = BLOCKS[version];
        let ecc_len = ECC_PER_BLOCK[version];
        let raw = raw_data_modules(version) / 8;
        let short_blocks = blocks - raw % blocks;
        let short_len = raw / blocks;
        let divisor = rs_divisor(ecc_len);
        let mut split = Vec::with_capacity(blocks);
        let mut at = 0;
        for i in 0..blocks {
            let length = short_len - ecc_len + usize::from(i >= short_blocks);
            let chunk = &data[at..at + length];
            at += length;
            let ecc = rs_remainder(chunk, &divisor);
            let mut block = chunk.to_vec();
            if i < short_blocks {
                block.push(0);
            }
            block.extend(ecc);
            split.push(block);
        }
        let mut result = Vec::with_capacity(raw);
        for i in 0..split[0].len() {
            for (j, block) in split.iter().enumerate() {
                if i != short_len - ecc_len || j >= short_blocks {
                    result.push(block[i]);
                }
            }
        }
        result
    }

    fn set_function(&mut self, x: usize, y: usize, dark: bool) {
        let index = y * self.size + x;
        self.modules[index] = dark;
        self.function[index] = true;
    }

    fn draw_function_patterns(&mut self) {
        let size = self.size;
        for i in 0..size {
            self.set_function(6, i, i % 2 == 0);
            self.set_function(i, 6, i % 2 == 0);
        }
        for (cx, cy) in [(3, 3), (size - 4, 3), (3, size - 4)] {
            for dy in -4i64..=4 {
                for dx in -4i64..=4 {
                    let (x, y) = (cx as i64 + dx, cy as i64 + dy);
                    if (0..size as i64).contains(&x) && (0..size as i64).contains(&y) {
                        let distance = dx.abs().max(dy.abs());
                        self.set_function(x as usize, y as usize, distance != 2 && distance != 4);
                    }
                }
            }
        }
        let positions = alignment_positions(self.version, size);
        let last = positions.len().saturating_sub(1);
        for (i, &px) in positions.iter().enumerate() {
            for (j, &py) in positions.iter().enumerate() {
                if (i == 0 && j == 0) || (i == 0 && j == last) || (i == last && j == 0) {
                    continue;
                }
                for dy in -2i64..=2 {
                    for dx in -2i64..=2 {
                        let distance = dx.abs().max(dy.abs());
                        self.set_function(
                            (px as i64 + dx) as usize,
                            (py as i64 + dy) as usize,
                            distance != 1,
                        );
                    }
                }
            }
        }
        // Reserve the format areas, and draw the version.
        self.draw_format(0);
        if self.version >= 7 {
            let mut remainder = self.version as u32;
            for _ in 0..12 {
                remainder = (remainder << 1) ^ ((remainder >> 11) * 0x1F25);
            }
            let bits = ((self.version as u32) << 12) | remainder;
            for i in 0..18 {
                let dark = (bits >> i) & 1 == 1;
                let a = size - 11 + i % 3;
                let b = i / 3;
                self.set_function(a, b, dark);
                self.set_function(b, a, dark);
            }
        }
    }

    fn format_bits(mask: u8) -> u32 {
        let data = (LEVEL_M_BITS << 3) | u32::from(mask);
        let mut remainder = data;
        for _ in 0..10 {
            remainder = (remainder << 1) ^ ((remainder >> 9) * 0x537);
        }
        ((data << 10) | remainder) ^ 0x5412
    }

    fn draw_format(&mut self, mask: u8) {
        let bits = Self::format_bits(mask);
        let bit = |i: usize| (bits >> i) & 1 == 1;
        let size = self.size;
        for i in 0..=5 {
            self.set_function(8, i, bit(i));
        }
        self.set_function(8, 7, bit(6));
        self.set_function(8, 8, bit(7));
        self.set_function(7, 8, bit(8));
        for i in 9..15 {
            self.set_function(14 - i, 8, bit(i));
        }
        for i in 0..8 {
            self.set_function(size - 1 - i, 8, bit(i));
        }
        for i in 8..15 {
            self.set_function(8, size - 15 + i, bit(i));
        }
        self.set_function(8, size - 8, true);
    }

    /// The positions of the data modules, in the order the codewords fill them.
    fn data_positions(&self) -> Vec<(usize, usize)> {
        let size = self.size;
        let mut positions = Vec::new();
        let mut right = size as i64 - 1;
        while right >= 1 {
            if right == 6 {
                right = 5;
            }
            for vertical in 0..size {
                for j in 0..2 {
                    let x = (right - j) as usize;
                    let upward = ((right + 1) & 2) == 0;
                    let y = if upward {
                        size - 1 - vertical
                    } else {
                        vertical
                    };
                    if !self.function[y * size + x] {
                        positions.push((x, y));
                    }
                }
            }
            right -= 2;
        }
        positions
    }

    fn draw_codewords(&mut self, codewords: &[u8]) {
        let positions = self.data_positions();
        for (i, (x, y)) in positions.into_iter().enumerate() {
            let dark = codewords
                .get(i / 8)
                .is_some_and(|byte| (byte >> (7 - i % 8)) & 1 == 1);
            self.modules[y * self.size + x] = dark;
        }
    }

    fn mask_bit(mask: u8, x: usize, y: usize) -> bool {
        match mask {
            0 => (x + y).is_multiple_of(2),
            1 => y.is_multiple_of(2),
            2 => x.is_multiple_of(3),
            3 => (x + y).is_multiple_of(3),
            4 => (x / 3 + y / 2).is_multiple_of(2),
            5 => x * y % 2 + x * y % 3 == 0,
            6 => (x * y % 2 + x * y % 3).is_multiple_of(2),
            _ => ((x + y) % 2 + x * y % 3).is_multiple_of(2),
        }
    }

    fn apply_mask(&mut self, mask: u8) {
        for y in 0..self.size {
            for x in 0..self.size {
                let index = y * self.size + x;
                if !self.function[index] && Self::mask_bit(mask, x, y) {
                    self.modules[index] = !self.modules[index];
                }
            }
        }
    }

    /// Whether the module at `x`, `y` is dark.
    #[must_use]
    pub fn dark(&self, x: usize, y: usize) -> bool {
        x < self.size && y < self.size && self.modules[y * self.size + x]
    }

    /// How many modules on each side.
    #[must_use]
    pub const fn size(&self) -> usize {
        self.size
    }

    fn penalty(&self) -> u32 {
        let size = self.size;
        let mut penalty = 0u32;
        let lines = |transpose: bool| -> Vec<Vec<bool>> {
            (0..size)
                .map(|a| {
                    (0..size)
                        .map(|b| {
                            if transpose {
                                self.dark(a, b)
                            } else {
                                self.dark(b, a)
                            }
                        })
                        .collect()
                })
                .collect()
        };
        let finder = [true, false, true, true, true, false, true];
        for line in lines(false).into_iter().chain(lines(true)) {
            // Rule 1: runs of five or more.
            let mut run = 1usize;
            for i in 1..=size {
                if i < size && line[i] == line[i - 1] {
                    run += 1;
                } else {
                    if run >= 5 {
                        penalty += 3 + (run as u32 - 5);
                    }
                    run = 1;
                }
            }
            // Rule 3: a pattern like a finder with four light modules on a side.
            for start in 0..size.saturating_sub(6) {
                if line[start..start + 7] == finder {
                    let before = start >= 4 && line[start - 4..start].iter().all(|dark| !dark);
                    let after =
                        start + 11 <= size && line[start + 7..start + 11].iter().all(|dark| !dark);
                    if before || after {
                        penalty += 40;
                    }
                }
            }
        }
        // Rule 2: blocks of 2x2 of one color.
        for y in 0..size - 1 {
            for x in 0..size - 1 {
                let color = self.dark(x, y);
                if color == self.dark(x + 1, y)
                    && color == self.dark(x, y + 1)
                    && color == self.dark(x + 1, y + 1)
                {
                    penalty += 3;
                }
            }
        }
        // Rule 4: the share of dark modules away from half.
        let dark = self.modules.iter().filter(|dark| **dark).count();
        let total = size * size;
        let k = ((dark * 20).abs_diff(total * 10))
            .div_ceil(total)
            .saturating_sub(1);
        penalty += k as u32 * 10;
        penalty
    }

    /// The code drawn with half blocks, two rows per line of text, with a quiet zone of four
    /// modules. Light modules are drawn: on a dark terminal, the code reads as dark on light.
    #[must_use]
    pub fn to_terminal(&self) -> String {
        let quiet = 4i64;
        let side = self.size as i64;
        let light = |x: i64, y: i64| {
            !(x >= 0 && y >= 0 && x < side && y < side && self.dark(x as usize, y as usize))
        };
        let mut text = String::new();
        let mut y = -quiet;
        while y < side + quiet {
            for x in -quiet..side + quiet {
                let shown = match (light(x, y), light(x, y + 1)) {
                    (true, true) => '\u{2588}',
                    (true, false) => '\u{2580}',
                    (false, true) => '\u{2584}',
                    (false, false) => ' ',
                };
                text.push(shown);
            }
            let _ = writeln!(text);
            y += 2;
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reads a code back the way a reader does: the format, the mask, the codewords, the
    /// blocks, their Reed-Solomon check, and the bytes.
    fn decode(code: &QrCode) -> Vec<u8> {
        let size = code.size;
        // The first copy of the format, with its BCH check.
        let mut bits = 0u32;
        for i in 0..=5 {
            bits |= u32::from(code.dark(8, i)) << i;
        }
        bits |= u32::from(code.dark(8, 7)) << 6;
        bits |= u32::from(code.dark(8, 8)) << 7;
        bits |= u32::from(code.dark(7, 8)) << 8;
        for i in 9..15 {
            bits |= u32::from(code.dark(14 - i, 8)) << i;
        }
        let mask = (0..8u8)
            .find(|mask| QrCode::format_bits(*mask) == bits)
            .expect("the format is one of the eight of level M");
        // The second copy says the same.
        let mut second = 0u32;
        for i in 0..8 {
            second |= u32::from(code.dark(size - 1 - i, 8)) << i;
        }
        for i in 8..15 {
            second |= u32::from(code.dark(8, size - 15 + i)) << i;
        }
        assert_eq!(second, bits);
        let positions = code.data_positions();
        let mut codewords = vec![0u8; positions.len() / 8];
        for (i, (x, y)) in positions.into_iter().enumerate().take(codewords.len() * 8) {
            let dark = code.dark(x, y) ^ QrCode::mask_bit(mask, x, y);
            codewords[i / 8] |= u8::from(dark) << (7 - i % 8);
        }
        let version = code.version;
        let blocks = BLOCKS[version];
        let ecc_len = ECC_PER_BLOCK[version];
        let raw = raw_data_modules(version) / 8;
        let short_blocks = blocks - raw % blocks;
        let short_len = raw / blocks;
        let mut split: Vec<Vec<u8>> = vec![Vec::new(); blocks];
        let mut at = 0;
        for i in 0..short_len + 1 {
            for (j, block) in split.iter_mut().enumerate() {
                if i == short_len - ecc_len && j < short_blocks {
                    continue;
                }
                block.push(codewords[at]);
                at += 1;
            }
        }
        let divisor = rs_divisor(ecc_len);
        let mut data = Vec::new();
        for block in &split {
            let (message, ecc) = block.split_at(block.len() - ecc_len);
            assert_eq!(
                rs_remainder(message, &divisor),
                ecc,
                "the Reed-Solomon check"
            );
            data.extend_from_slice(message);
        }
        let bit = |i: usize| (data[i / 8] >> (7 - i % 8)) & 1;
        let read = |from: usize, length: usize| {
            (from..from + length).fold(0usize, |v, i| (v << 1) | usize::from(bit(i)))
        };
        assert_eq!(read(0, 4), 0b0100, "byte mode");
        let count_bits = if version <= 9 { 8 } else { 16 };
        let length = read(4, count_bits);
        (0..length)
            .map(|k| read(4 + count_bits + k * 8, 8) as u8)
            .collect()
    }

    #[test]
    fn the_tables_give_the_known_capacities() {
        // Data codewords of level M in the standard.
        assert_eq!(data_codewords(1), 16);
        assert_eq!(data_codewords(5), 86);
        assert_eq!(data_codewords(10), 216);
        assert_eq!(data_codewords(20), 669);
        assert_eq!(data_codewords(40), 2_334);
        assert_eq!(alignment_positions(7, 45), [6, 22, 38]);
        assert_eq!(alignment_positions(32, 145), [6, 34, 60, 86, 112, 138]);
    }

    #[test]
    fn a_code_reads_back_as_what_was_encoded() {
        let mut texts: Vec<Vec<u8>> = vec![b"a".to_vec(), b"HELLO WORLD".to_vec()];
        // The size of what a pairing holds, and more, through several versions.
        for length in [40, 100, 160, 240, 300, 600, 1_200] {
            texts.push((0..length).map(|i| b"2@AbC,+/=0123xyz"[i % 16]).collect());
        }
        for text in texts {
            let code = QrCode::encode(&text).unwrap();
            assert_eq!(code.size(), code.version * 4 + 17);
            assert_eq!(decode(&code), text, "{} bytes", text.len());
        }
    }

    #[test]
    fn the_finder_patterns_are_in_the_three_corners() {
        let code = QrCode::encode(b"pairing").unwrap();
        let last = code.size() - 1;
        for (x, y) in [
            (0, 0),
            (last, 0),
            (0, last),
            (3, 3),
            (last - 3, 3),
            (3, last - 3),
        ] {
            assert!(code.dark(x, y), "{x}, {y}");
        }
        assert!(!code.dark(1, 1) && !code.dark(7, 7));
        assert!(code.dark(8, code.size() - 8), "the dark module");
    }

    #[test]
    fn too_long_is_refused_and_the_terminal_drawing_has_the_quiet_zone() {
        assert_eq!(QrCode::encode(&vec![b'x'; 3_000]), Err(TooLong));
        let code = QrCode::encode(b"x").unwrap();
        let drawing = code.to_terminal();
        let lines: Vec<&str> = drawing.lines().collect();
        assert_eq!(lines.len(), (code.size() + 8).div_ceil(2));
        assert!(
            lines
                .iter()
                .all(|line| line.chars().count() == code.size() + 8)
        );
        assert!(lines[0].chars().all(|c| c == '\u{2588}'));
    }
}
