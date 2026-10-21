// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! kun: what shaping a layout's text gave, kept for the next layout with
//! the same text, styled the same in what changes shaping, with its inline
//! boxes in the same places and the same analysis: a browser shapes the
//! same text again as content is built again, and alike text in alike
//! elements (a list's items, a page's repeated parts). Its other styles
//! (colors, decorations, line height, line breaking) may differ: each
//! character takes the new layout's style.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::hash::{BuildHasher, Hash, Hasher};

use fontique::{FamilyId, FontStyle, FontWeight, FontWidth};
use hashbrown::HashMap;
use parlance::BidiLevel;
use parley_engine::{Analysis, CharInfo, ShapedText};

use super::super::resolve::{ResolveContext, ResolvedStyle};
use super::super::style::{Brush, FontFeature, FontVariation};
use crate::inline_box::LayoutInlineBox;

/// How many characters of shaped text the cache keeps before it starts
/// again.
const CHARACTERS: usize = 1 << 18;

/// Shaped texts by what they were shaped with.
#[derive(Default)]
pub(crate) struct ShapeCache {
    entries: HashMap<u64, Vec<Entry>>,
    characters: usize,
    /// How many layouts took a kept shaping (for tests).
    pub(crate) reused: usize,
}

/// What shaping a text depends on but the text.
#[derive(Clone, PartialEq)]
struct RunStyle {
    families: Vec<FamilyId>,
    font_size: u32,
    font_width: FontWidth,
    font_style: FontStyle,
    font_weight: FontWeight,
    variations: Vec<FontVariation>,
    features: Vec<FontFeature>,
    locale: Option<parlance::Language>,
    letter_spacing: u32,
    word_spacing: u32,
    optical_sizing: bool,
}

impl RunStyle {
    fn of<B: Brush>(rcx: &ResolveContext, style: &ResolvedStyle<B>) -> Self {
        Self {
            families: rcx.stack(style.font_family).unwrap_or(&[]).to_vec(),
            font_size: style.font_size.to_bits(),
            font_width: style.font_width,
            font_style: style.font_style,
            font_weight: style.font_weight,
            variations: rcx
                .variations(style.font_variations)
                .unwrap_or(&[])
                .to_vec(),
            features: rcx.features(style.font_features).unwrap_or(&[]).to_vec(),
            locale: style.locale,
            letter_spacing: style.letter_spacing.to_bits(),
            word_spacing: style.word_spacing.to_bits(),
            optical_sizing: style.optical_sizing,
        }
    }
}

/// A layout's text and what its shaping depends on.
struct Entry {
    text: Box<str>,
    /// Each run of characters styled alike: where it ends, and its style.
    runs: Vec<(usize, RunStyle)>,
    boxes: Vec<usize>,
    char_info: Vec<CharInfo>,
    bidi_levels: Vec<BidiLevel>,
    paragraph_level: BidiLevel,
    shaped: ShapedText,
}

/// What a layout is shaped with, as `shape_text` has it.
pub(crate) struct Key<'a> {
    text: &'a str,
    runs: Vec<(usize, RunStyle)>,
    boxes: Vec<usize>,
    analysis: &'a Analysis,
    hash: u64,
}

impl<'a> Key<'a> {
    pub(crate) fn new<B: Brush>(
        rcx: &ResolveContext,
        styles: &[ResolvedStyle<B>],
        inline_boxes: &[LayoutInlineBox],
        analysis: &'a Analysis,
        char_style_indices: &[u16],
        text: &'a str,
        hasher: &impl BuildHasher,
    ) -> Self {
        let run_styles: Vec<RunStyle> = styles
            .iter()
            .map(|style| RunStyle::of(rcx, style))
            .collect();
        let mut runs: Vec<(usize, RunStyle)> = Vec::new();
        for (i, &style_index) in char_style_indices.iter().enumerate() {
            let style = &run_styles[usize::from(style_index)];
            match runs.last_mut() {
                Some((end, last)) if last == style => *end = i + 1,
                _ => runs.push((i + 1, style.clone())),
            }
        }
        let boxes: Vec<usize> = inline_boxes.iter().map(|b| b.inline_box.index).collect();
        let mut state = hasher.build_hasher();
        text.hash(&mut state);
        boxes.hash(&mut state);
        for (end, style) in &runs {
            end.hash(&mut state);
            style.families.hash(&mut state);
            style.font_size.hash(&mut state);
        }
        Self {
            text,
            runs,
            boxes,
            analysis,
            hash: state.finish(),
        }
    }
}

impl Entry {
    fn matches(&self, key: &Key<'_>) -> bool {
        &*self.text == key.text
            && self.boxes == key.boxes
            && self.runs == key.runs
            && self.char_info == key.analysis.char_info()
            && self.bidi_levels == key.analysis.bidi_levels()
            && self.paragraph_level == key.analysis.paragraph_level()
    }
}

impl ShapeCache {
    /// The hasher keys are made with.
    pub(crate) fn hasher(&self) -> &impl BuildHasher {
        self.entries.hasher()
    }

    /// The shaping kept for `key`, if any.
    pub(crate) fn get(&self, key: &Key<'_>) -> Option<&ShapedText> {
        self.entries
            .get(&key.hash)?
            .iter()
            .find(|entry| entry.matches(key))
            .map(|entry| &entry.shaped)
    }

    /// Keeps `shaped`, what shaping `key` gave.
    pub(crate) fn insert(&mut self, key: Key<'_>, shaped: &ShapedText) {
        let characters = key.analysis.char_info().len();
        if self.characters + characters > CHARACTERS {
            self.entries.clear();
            self.characters = 0;
        }
        self.characters += characters;
        self.entries.entry(key.hash).or_default().push(Entry {
            text: key.text.into(),
            runs: key.runs,
            boxes: key.boxes,
            char_info: key.analysis.char_info().to_vec(),
            bidi_levels: key.analysis.bidi_levels().to_vec(),
            paragraph_level: key.analysis.paragraph_level(),
            shaped: shaped.clone(),
        });
    }
}

/// Whether shapings `a` and `b` are the same, but for which loaded copy of
/// a font file each holds: fontique loads a file again once nothing holds
/// the copy it had, and a kept shaping holds the copy it was made with.
#[cfg(debug_assertions)]
pub(crate) fn same_shaping(a: &ShapedText, b: &ShapedText) -> bool {
    a.runs() == b.runs()
        && a.characters() == b.characters()
        && a.shaped_clusters() == b.shaped_clusters()
        && a.glyphs() == b.glyphs()
        && a.normalized_coords() == b.normalized_coords()
        && a.fonts().len() == b.fonts().len()
        && a.fonts().iter().zip(b.fonts()).all(|(a, b)| {
            let (data_a, data_b) = (a.font.data.data(), b.font.data.data());
            a.font.index == b.font.index
                && a.synthesis == b.synthesis
                && (core::ptr::eq(data_a, data_b) || data_a == data_b)
        })
}
