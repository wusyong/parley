// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! kun: line break opportunities looked up in a table for characters up to
//! U+017F and from U+2000 to U+203F (Latin-1, Latin Extended-A and the
//! commonest punctuation), giving what ICU4X's line segmenter gives under the
//! default options; the words the table can't decide are given to ICU4X.

use alloc::vec::Vec;

use icu_segmenter::LineSegmenterBorrowed;

/// No opportunity.
const N: u8 = 0;
/// An opportunity.
const B: u8 = 1;
/// The table can't decide: it depends on more than the two characters, or
/// on a character outside the table.
const S: u8 = 2;

// GENERATED START
// Printed by `tables_are_up_to_date` when they aren't.

/// How many classes there are, with 0 for those the table doesn't decide.
const CLASS_COUNT: usize = 23;

/// [`ENTRIES`]'s previous class at the text's start.
const SOT: u8 = 23;

/// The class of each character up to U+017F:
/// 1: BA ('\t'),
/// 2: EX ('!'),
/// 3: QU ('"'),
/// 4: AL ('#'),
/// 5: PR ('$'),
/// 6: PO ('%'),
/// 7: OP ('('),
/// 8: CP (')'),
/// 9: IS (','),
/// 10: HY ('-'),
/// 11: SY ('/'),
/// 12: NU ('0'),
/// 13: CL ('}'),
/// 14: GL ('\u{a0}'),
/// 15: AI ('§'),
/// 16: QU Pi ('«'),
/// 17: BB ('´'),
/// 18: QU Pf ('»'),
/// 19: HH ('‐'),
/// 20: B2 ('—'),
/// 21: IN ('․'),
/// 22: NS ('‼'),
/// 0: the others.
#[rustfmt::skip]
const CLASSES: [u8; 0x180] = [
     0,  0,  0,  0,  0,  0,  0,  0,  0,  1,  0,  0,  0,  0,  0,  0,
     0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
     0,  2,  3,  4,  5,  6,  4,  3,  7,  8,  4,  5,  9, 10,  9, 11,
    12, 12, 12, 12, 12, 12, 12, 12, 12, 12,  9,  9,  4,  4,  4,  2,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  7,  5,  8,  4,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  7,  1, 13,  4,  0,
     0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
     0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,  0,
    14,  7,  6,  5,  5,  5,  4, 15, 15,  4, 15, 16,  4,  1,  4,  4,
     6,  5, 15, 15, 17,  4, 15, 15, 15, 15, 15, 18, 15, 15, 15,  7,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4, 15,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4, 15,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,
     4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,
];

/// The class of each character from U+2000 to U+203F.
#[rustfmt::skip]
const PUNCTUATION_CLASSES: [u8; 0x40] = [
     1,  1,  1,  1,  1,  1,  1, 14,  1,  1,  1,  0,  0,  0,  0,  0,
    19, 14, 19, 19, 20, 15, 15,  4, 16, 18,  7, 16, 16, 18,  7, 16,
    15, 15,  4,  4, 21, 21, 21,  1,  0,  0,  0,  0,  0,  0,  0, 14,
     6,  6,  6,  6,  6,  6,  6,  6,  4, 16, 18, 15, 22, 22,  4,  4,
];

/// Whether there is an opportunity before a character, by the class of the
/// last character before it that isn't a space (or [`SOT`]), whether there
/// are spaces between them, and its class.
#[rustfmt::skip]
const ENTRIES: [[[u8; CLASS_COUNT]; 2]; CLASS_COUNT + 1] = [
    // 0
    [
        [S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S],
        [S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S, S],
    ],
    // 1: BA
    [
        [S, N, N, N, B, B, B, B, N, N, N, N, B, N, B, B, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 2: EX
    [
        [S, N, N, N, B, B, B, B, N, N, N, N, B, N, N, B, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 3: QU
    [
        [S, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 4: AL
    [
        [S, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 5: PR
    [
        [S, N, N, N, N, B, B, N, N, N, N, N, N, N, N, N, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 6: PO
    [
        [S, N, N, N, N, B, B, N, N, N, N, N, N, N, N, N, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 7: OP
    [
        [S, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N],
        [S, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N],
    ],
    // 8: CP
    [
        [S, N, N, N, N, N, N, B, N, N, N, N, N, N, N, N, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, N],
    ],
    // 9: IS
    [
        [S, N, N, N, N, B, B, B, N, N, N, N, N, N, N, N, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 10: HY
    [
        [S, N, N, N, B, B, B, B, N, N, N, N, N, N, B, B, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 11: SY
    [
        [S, N, N, N, B, B, B, B, N, N, N, N, N, N, N, B, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 12: NU
    [
        [S, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 13: CL
    [
        [S, N, N, N, B, N, N, B, N, N, N, N, B, N, N, B, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, N],
    ],
    // 14: GL
    [
        [S, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 15: AI
    [
        [S, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 16: QU Pi
    [
        [S, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N],
        [S, S, N, S, S, S, S, S, N, N, S, N, S, N, S, S, S, S, S, S, S, S, S],
    ],
    // 17: BB
    [
        [S, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 18: QU Pf
    [
        [S, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 19: HH
    [
        [S, N, N, N, B, B, B, B, N, N, N, N, B, N, B, B, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 20: B2
    [
        [S, N, N, N, B, B, B, B, N, N, N, N, B, N, N, B, N, B, N, N, N, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, N, B, B],
    ],
    // 21: IN
    [
        [S, N, N, N, B, B, B, B, N, N, N, N, B, N, N, B, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // 22: NS
    [
        [S, N, N, N, B, B, B, B, N, N, N, N, B, N, N, B, N, B, N, N, B, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
    // the text's start
    [
        [N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N, N],
        [S, B, N, B, B, B, B, B, N, N, B, N, B, N, B, B, B, B, S, B, B, B, B],
    ],
];
// GENERATED END

/// The class of a character in [`ENTRIES`]; 0 when it is outside the table
/// or of a class the table doesn't decide.
#[inline(always)]
fn class_of(c: char) -> u8 {
    let code = u32::from(c);
    let (classes, first): (&[u8], u32) = match code {
        0..0x180 => (&CLASSES, 0),
        0x2000..0x2040 => (&PUNCTUATION_CLASSES, 0x2000),
        _ => return 0,
    };
    usize::try_from(code - first).map_or(0, |index| classes[index])
}

/// Appends the line break opportunities in `text` that ICU4X's line
/// segmenter (`segmenter`, with the default options) gives, but those at its
/// start and end; returns how many words (text between spaces) were given to
/// ICU4X as the table couldn't decide them.
///
/// The opportunity before a character depends on the last character that
/// isn't a space before it, whether there are spaces between them, and the
/// character, as in UAX #14's pair table: the table holds what ICU4X gives
/// for each of these where that doesn't change with the characters around
/// them. There is never one before a space (LB7) but after a character
/// outside the table, so only the opportunity at a word's start and those
/// in it are looked up.
pub(crate) fn line_breaks(
    text: &str,
    segmenter: LineSegmenterBorrowed<'_>,
    out: &mut Vec<usize>,
) -> usize {
    let bytes = text.as_bytes();
    let mut given = 0;
    let mut prev = SOT;
    let mut spaces = false;
    let mut prev_word_start = None;
    let mut at = 0;
    loop {
        while bytes.get(at) == Some(&b' ') {
            at += 1;
            spaces = true;
        }
        if at == bytes.len() {
            return given;
        }
        let start = at;
        let mark = out.len();
        let mut decided = true;
        while let Some(&byte) = bytes.get(at) {
            if byte == b' ' {
                break;
            }
            // The character's class and length: one byte up to U+007F, two
            // (0xC2 to 0xC5, then one) up to U+017F, three (0xE2, 0x80, then
            // one) from U+2000 to U+203F.
            let (class, length) = match byte {
                0..0x80 => (CLASSES[usize::from(byte)], 1),
                0xC2..=0xC5 => match bytes.get(at + 1) {
                    Some(&next) => {
                        let index = (usize::from(byte & 0x7) << 6) | usize::from(next & 0x3F);
                        (CLASSES[index], 2)
                    }
                    None => (0, 1),
                },
                0xE2 => match bytes.get(at + 1..at + 3) {
                    Some(&[0x80, last]) => (PUNCTUATION_CLASSES[usize::from(last & 0x3F)], 3),
                    _ => (0, 1),
                },
                _ => (0, 1),
            };
            // A character outside the table may make an opportunity after
            // it, or before the next word, even at the text's start (where
            // the table has none).
            decided = class != 0
                && match ENTRIES[usize::from(prev)][usize::from(spaces)][usize::from(class)] {
                    N => true,
                    B => {
                        out.push(at);
                        true
                    }
                    _ => false,
                };
            if !decided {
                // The rest of the word is only looked through for its end,
                // and its last character, which the next word looks up with.
                at += bytes[at..]
                    .iter()
                    .position(|&byte| byte == b' ')
                    .unwrap_or(bytes.len() - at);
                prev = text[start..at].chars().next_back().map_or(0, class_of);
                break;
            }
            prev = class;
            spaces = false;
            at += length;
        }
        let end = at;
        let next_start = bytes[end..]
            .iter()
            .position(|&byte| byte != b' ')
            .map_or(bytes.len(), |spaces| end + spaces);
        if !decided {
            // ICU4X on the word with the words on either side, which hold
            // what the rules look at around it, keeping the opportunities
            // from its start up to the next word's.
            out.truncate(mark);
            given += 1;
            let window_start = prev_word_start.unwrap_or(0);
            let window_end = bytes[next_start..]
                .iter()
                .position(|&byte| byte == b' ')
                .map_or(bytes.len(), |word| next_start + word);
            out.extend(
                segmenter
                    .segment_str(&text[window_start..window_end])
                    .map(|position| window_start + position)
                    .filter(|&position| {
                        position >= start && position < next_start && position != 0
                    }),
            );
        }
        prev_word_start = Some(start);
        at = end;
    }
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;
    use alloc::format;
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;
    use std::println;

    use icu_properties::props::{GeneralCategory, LineBreak};
    use icu_properties::{CodePointMapData, PropertyNamesShort};

    use super::{B, CLASS_COUNT, CLASSES, ENTRIES, N, PUNCTUATION_CLASSES, S, SOT, line_breaks};
    use crate::analysis::default_line_segmenter;

    /// What ICU4X gives for `text`, but the opportunities at its start and
    /// end.
    fn icu(text: &str) -> Vec<usize> {
        default_line_segmenter()
            .segment_str(text)
            .filter(|&position| position != 0 && position != text.len())
            .collect()
    }

    fn looked_up(text: &str) -> Vec<usize> {
        let mut out = Vec::new();
        line_breaks(text, default_line_segmenter(), &mut out);
        out
    }

    /// A character's line break class, with quotation marks split into
    /// opening, closing and the others (UAX #14's rules 15a, 15b and 19 tell
    /// them apart), or `None` for a space, which the lookup handles itself, and
    /// for a class the table doesn't decide: those that take the class of
    /// what's before them (CM, ZWJ), mandatory breaks, ZW, and those
    /// resolved some other way (SA, SG, XX, CB).
    fn key(c: char) -> Option<(LineBreak, Option<GeneralCategory>)> {
        let class = CodePointMapData::<LineBreak>::new().get(c);
        let slow = [
            LineBreak::SP,
            LineBreak::CM,
            LineBreak::ZWJ,
            LineBreak::BK,
            LineBreak::CR,
            LineBreak::LF,
            LineBreak::NL,
            LineBreak::ZW,
            LineBreak::SA,
            LineBreak::SG,
            LineBreak::XX,
            LineBreak::CB,
        ];
        if slow.contains(&class) {
            return None;
        }
        let category = CodePointMapData::<GeneralCategory>::new().get(c);
        let quote = matches!(
            category,
            GeneralCategory::InitialPunctuation | GeneralCategory::FinalPunctuation
        );
        Some((class, (class == LineBreak::QU && quote).then_some(category)))
    }

    struct Tables {
        /// The first character of each class, from 1.
        representatives: Vec<char>,
        /// Up to U+017F.
        classes: Vec<u8>,
        /// From U+2000 to U+203F.
        punctuation: Vec<u8>,
        entries: Vec<[Vec<u8>; 2]>,
    }

    /// Characters tried around each pair: besides one of each class, the
    /// text's start and end, a space, characters of the classes the table
    /// doesn't decide, and some outside the table.
    fn contexts(representatives: &[char]) -> Vec<String> {
        let mut contexts = vec![String::new(), String::from(" ")];
        contexts.extend(representatives.iter().skip(1).map(|&c| String::from(c)));
        contexts.extend(
            [
                '\n', '\r', '\u{b}', '\u{85}', '\u{1}', '\u{301}', '\u{200d}', '\u{200b}',
                '\u{2028}', '\u{2060}', '\u{20ac}', '\u{3001}', '中',
            ]
            .map(String::from),
        );
        contexts
    }

    fn generate() -> Tables {
        let mut keys = Vec::new();
        let mut representatives = vec!['\0'];
        let mut classes = Vec::new();
        let mut punctuation = Vec::new();
        for code in (0..0x180).chain(0x2000..0x2040) {
            let c = char::from_u32(code).unwrap();
            let class = match key(c) {
                None => 0,
                Some(key) => match keys.iter().position(|&known| known == key) {
                    Some(index) => index + 1,
                    None => {
                        keys.push(key);
                        representatives.push(c);
                        keys.len()
                    }
                },
            };
            let class = u8::try_from(class).unwrap();
            if code < 0x180 {
                classes.push(class);
            } else {
                punctuation.push(class);
            }
        }
        let count = representatives.len();
        let contexts = contexts(&representatives);
        let mut entries = Vec::new();
        for prev in 0..=count {
            let mut row = [vec![S; count], vec![S; count]];
            for (spaces, row) in row.iter_mut().enumerate() {
                for (class, entry) in row.iter_mut().enumerate() {
                    let gap = if spaces == 1 { " " } else { "" };
                    if prev == count && spaces == 0 {
                        // No opportunity at the text's start.
                        *entry = N;
                        continue;
                    }
                    if prev == 0 || class == 0 {
                        continue;
                    }
                    let mut seen = BTreeMap::new();
                    let before: &[String] = if prev == count {
                        &[String::new()]
                    } else {
                        &contexts
                    };
                    for before in before {
                        for after in &contexts {
                            let mut text = before.clone();
                            if prev != count {
                                text.push(representatives[prev]);
                            }
                            text.push_str(gap);
                            let position = text.len();
                            text.push(representatives[class]);
                            text.push_str(after);
                            *seen.entry(icu(&text).contains(&position)).or_insert(0) += 1;
                        }
                    }
                    *entry = match (seen.get(&false), seen.get(&true)) {
                        (Some(_), None) => N,
                        (None, Some(_)) => B,
                        _ => S,
                    };
                }
            }
            entries.push(row);
        }
        Tables {
            representatives,
            classes,
            punctuation,
            entries,
        }
    }

    fn name(c: char) -> String {
        let (class, quote) = key(c).unwrap();
        let name = PropertyNamesShort::<LineBreak>::new().get(class).unwrap();
        match quote {
            Some(GeneralCategory::InitialPunctuation) => format!("{name} Pi"),
            Some(_) => format!("{name} Pf"),
            None => String::from(name),
        }
    }

    fn source(tables: &Tables) -> String {
        let count = tables.representatives.len();
        let mut source = String::from(
            "// GENERATED START\n\
             // Printed by `tables_are_up_to_date` when they aren't.\n\n\
             /// How many classes there are, with 0 for those the table doesn't decide.\n",
        );
        source += &format!("const CLASS_COUNT: usize = {count};\n\n");
        source += "/// [`ENTRIES`]'s previous class at the text's start.\n";
        source += &format!("const SOT: u8 = {count};\n\n");
        source += "/// The class of each character up to U+017F:\n";
        for (class, &c) in tables.representatives.iter().enumerate().skip(1) {
            source += &format!("/// {class}: {} ({c:?}),\n", name(c));
        }
        source += "/// 0: the others.\n#[rustfmt::skip]\nconst CLASSES: [u8; 0x180] = [\n";
        let rows = |classes: &[u8]| {
            let mut source = String::new();
            for row in classes.chunks(16) {
                let row: Vec<String> = row.iter().map(|class| format!("{class:2},")).collect();
                source += &format!("    {}\n", row.join(" "));
            }
            source
        };
        source += &rows(&tables.classes);
        source += "];\n\n";
        source += "/// The class of each character from U+2000 to U+203F.\n\
                   #[rustfmt::skip]\n\
                   const PUNCTUATION_CLASSES: [u8; 0x40] = [\n";
        source += &rows(&tables.punctuation);
        source += "];\n\n";
        source += "/// Whether there is an opportunity before a character, by the class of the\n\
                   /// last character before it that isn't a space (or [`SOT`]), whether there\n\
                   /// are spaces between them, and its class.\n\
                   #[rustfmt::skip]\n";
        source += "const ENTRIES: [[[u8; CLASS_COUNT]; 2]; CLASS_COUNT + 1] = [\n";
        for (prev, row) in tables.entries.iter().enumerate() {
            let label = if prev == count {
                String::from("the text's start")
            } else if prev == 0 {
                String::from("0")
            } else {
                format!("{prev}: {}", name(tables.representatives[prev]))
            };
            source += &format!("    // {label}\n    [\n");
            for row in row {
                let row: Vec<&str> = row
                    .iter()
                    .map(|&entry| match entry {
                        N => "N",
                        B => "B",
                        _ => "S",
                    })
                    .collect();
                source += &format!("        [{}],\n", row.join(", "));
            }
            source += "    ],\n";
        }
        source += "];\n// GENERATED END\n";
        source
    }

    #[test]
    #[expect(
        clippy::print_stdout,
        reason = "prints the tables anew when out of date"
    )]
    fn tables_are_up_to_date() {
        let tables = generate();
        let entries: Vec<[Vec<u8>; 2]> = ENTRIES
            .iter()
            .map(|row| [row[0].to_vec(), row[1].to_vec()])
            .collect();
        let current = tables.representatives.len() == CLASS_COUNT
            && usize::from(SOT) == CLASS_COUNT
            && tables.classes == CLASSES
            && tables.punctuation == PUNCTUATION_CLASSES
            && tables.entries == entries;
        if !current {
            println!("{}", source(&tables));
        }
        assert!(
            current,
            "the tables are out of date: replace them with the source printed"
        );
    }

    /// Every text of up to four characters of one of each class, a space, a
    /// character of a class the table doesn't decide and one outside it.
    #[test]
    fn short_texts_break_as_icu4x_breaks_them() {
        let tables = generate();
        let mut alphabet = tables.representatives[1..].to_vec();
        alphabet.extend([' ', '\n', '\u{2060}']);
        let mut texts = vec![String::new()];
        for _ in 0..4 {
            texts = texts
                .iter()
                .flat_map(|text| {
                    alphabet.iter().map(move |&c| {
                        let mut text = text.clone();
                        text.push(c);
                        text
                    })
                })
                .collect();
            for text in &texts {
                assert_eq!(looked_up(text), icu(text), "{text:?}");
            }
        }
    }

    /// Long texts of characters in the table, mostly letters and spaces,
    /// with some outside it.
    #[test]
    fn long_texts_break_as_icu4x_breaks_them() {
        let outside = [
            '\u{2060}',
            '\u{20ac}',
            '\u{301}',
            '\u{3001}',
            '中',
            '\u{1f600}',
        ];
        // xorshift64, for the same texts every time.
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..50_000 {
            let length = next() % 48;
            let text: String = (0..length)
                .map(|_| match next() % 32 {
                    0..=7 => ' ',
                    8..=15 => 'a',
                    16 => outside[usize::try_from(next() % 6).unwrap()],
                    17..=20 => {
                        char::from_u32(0x2000 + u32::try_from(next() % 0x40).unwrap()).unwrap()
                    }
                    _ => char::from_u32(u32::try_from(next() % 0x180).unwrap()).unwrap(),
                })
                .collect();
            assert_eq!(looked_up(&text), icu(&text), "{text:?}");
        }
    }
}
