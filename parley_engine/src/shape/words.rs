// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! kun: what shaping a word gave, kept for the next time the same word is
//! shaped alike: text is mostly the same words again, and shaping one
//! again (picking its fonts, running harfrust) is most of the time text
//! takes. Gecko's `gfxShapedWord` cache and `WebKit`'s `WidthCache` keep
//! words too; Blink keeps whole runs.
//!
//! A segment's text is split into words before each space and after it,
//! never inside a grapheme. A word takes the fonts and glyphs kept for
//! it only where shaping it on its own gives what shaping the text whole
//! does: in a segment set left to right, in a script whose shaping
//! doesn't look at the text around the run (harfrust's Arabic shaper,
//! for Arabic and Syriac, does), without features that are off unless
//! asked for, and with fonts that don't shape a space together with what
//! is around it but the glyph beside it ([`space_reach`]): where a space
//! and the glyph beside it are shaped together, the two words are shaped
//! again as one. Font selection gives a grapheme the same font wherever it
//! is, given its style, script and language.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::hash::BuildHasher;
use core::ops::Range;

use hashbrown::{DefaultHashBuilder, HashMap, HashTable};
use parlance::{FontVariation, Language, Script};

use crate::FontInstance;
use crate::ShapeOptions;
use crate::shape::shaped_text::HarfGlyph;
use crate::shape::space::{SpaceReach, asks_for_rare_features, space_reach};

/// The longest word kept, in characters: longer ones are seldom the same
/// again.
pub(crate) const MAX_WORD_CHARS: usize = 32;

/// How many words are kept before the cache starts again.
const MAX_WORDS: usize = 1 << 15;

/// How many alike sets of options words are kept for before the cache
/// starts again.
const MAX_OPTIONS: usize = 1 << 12;

/// Words by their text, the font styles of their characters and what they
/// were shaped with.
pub(crate) struct Words {
    /// Whether segments are shaped a word at a time.
    pub(crate) enabled: bool,
    words: HashTable<Word>,
    hasher: DefaultHashBuilder,
    /// What words are shaped with, but their text and styles.
    options: HashMap<OptionsKey, u32>,
    /// How far shaping with each font, by its data's id and index, reaches
    /// across a space.
    fonts: HashMap<(u64, u32), SpaceReach>,
    /// How many words were taken from the cache (for tests).
    pub(crate) reused: usize,
    /// How many words were shaped and kept (for tests).
    pub(crate) shaped: usize,
    /// How many times two words were shaped again as one (for tests).
    pub(crate) joined: usize,
}

/// What shaping a word gave.
struct Word {
    hash: u64,
    options: u32,
    text: Box<str>,
    styles: Box<[u32]>,
    runs: Box<[WordRun]>,
    glyphs: Box<[HarfGlyph]>,
}

/// A run of a word's characters taking the same font.
pub(crate) struct WordRun {
    pub(crate) font: FontInstance,
    pub(crate) bytes: usize,
    pub(crate) chars: usize,
    /// Its glyphs, as harfrust gives them for the run on its own (clusters
    /// count from the run's first character).
    pub(crate) glyphs: Range<usize>,
    pub(crate) coords: Box<[harfrust::NormalizedCoord]>,
}

/// What a word is shaped with, but its text and the styles of its
/// characters.
#[derive(PartialEq, Eq, Hash)]
struct OptionsKey {
    script: Script,
    font_size: u32,
    language: Option<Language>,
    features: Vec<(u32, u16)>,
    variations: Vec<(u32, u32)>,
}

impl Default for Words {
    fn default() -> Self {
        Self {
            enabled: true,
            words: HashTable::new(),
            hasher: DefaultHashBuilder::default(),
            options: HashMap::new(),
            fonts: HashMap::new(),
            reused: 0,
            shaped: 0,
            joined: 0,
        }
    }
}

impl Words {
    /// Whether a segment can be shaped a word at a time, before its fonts
    /// are known.
    pub(crate) fn may_split(&self, ltr: bool, script: Script, options: &ShapeOptions<'_>) -> bool {
        const ARABIC: Script = Script::from_bytes(*b"Arab");
        const SYRIAC: Script = Script::from_bytes(*b"Syrc");
        self.enabled
            && ltr
            && script != ARABIC
            && script != SYRIAC
            && !asks_for_rare_features(options.features)
    }

    /// An id for what a segment is shaped with, but its text: its script
    /// and options, with `variations` (the options' and optical sizing's).
    pub(crate) fn options_id(
        &mut self,
        script: Script,
        options: &ShapeOptions<'_>,
        variations: &[FontVariation],
    ) -> u32 {
        let key = OptionsKey {
            script,
            font_size: options.font_size.to_bits(),
            language: options.language,
            features: options
                .features
                .iter()
                .map(|feature| (u32::from_be_bytes(feature.tag.to_bytes()), feature.value))
                .collect(),
            variations: variations
                .iter()
                .map(|variation| {
                    (
                        u32::from_be_bytes(variation.tag.to_bytes()),
                        variation.value.to_bits(),
                    )
                })
                .collect(),
        };
        if let Some(&id) = self.options.get(&key) {
            return id;
        }
        // Ids start again with the words: no word is kept with an id that
        // is given out again.
        if self.options.len() >= MAX_OPTIONS {
            self.options.clear();
            self.words.clear();
        }
        let id = u32::try_from(self.options.len()).expect("options are fewer than MAX_OPTIONS");
        self.options.insert(key, id);
        id
    }

    /// How far shaping with `font` reaches across a space.
    pub(crate) fn reach(&mut self, font: &FontInstance) -> &SpaceReach {
        let key = (font.font.data.id(), font.font.index);
        self.fonts.entry(key).or_insert_with(|| {
            skrifa::FontRef::from_index(font.font.data.as_ref(), font.font.index)
                .map_or(SpaceReach::Across, |font| space_reach(&font))
        })
    }

    /// How far shaping with `font` reaches across a space, if that was
    /// found out: it was for the fonts of every word kept.
    pub(crate) fn known_reach(&self, font: &FontInstance) -> Option<&SpaceReach> {
        self.fonts.get(&(font.font.data.id(), font.font.index))
    }

    /// The runs and glyphs kept for `text` with `styles`, shaped with
    /// `options`.
    pub(crate) fn get(
        &self,
        options: u32,
        text: &str,
        styles: &[u32],
    ) -> Option<(&[WordRun], &[HarfGlyph])> {
        let hash = self.hasher.hash_one((options, text, styles));
        self.words
            .find(hash, |word| {
                word.options == options && &*word.text == text && &*word.styles == styles
            })
            .map(|word| (&*word.runs, &*word.glyphs))
    }

    /// Keeps what shaping `text` with `styles` and `options` gave.
    pub(crate) fn insert(
        &mut self,
        options: u32,
        text: &str,
        styles: &[u32],
        runs: Vec<WordRun>,
        glyphs: &[HarfGlyph],
    ) {
        if self.words.len() >= MAX_WORDS {
            self.words.clear();
        }
        let hash = self.hasher.hash_one((options, text, styles));
        self.words.insert_unique(
            hash,
            Word {
                hash,
                options,
                text: text.into(),
                styles: styles.into(),
                runs: runs.into(),
                glyphs: glyphs.into(),
            },
            |word| word.hash,
        );
    }

    /// Forgets every word, and what was found out about fonts: fonts were
    /// added, so font selection may pick others.
    pub(crate) fn clear(&mut self) {
        self.words.clear();
        self.options.clear();
        self.fonts.clear();
    }
}
