// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Shaping of text.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ops::Range;
use harfrust::ShapeOptions as HarfShapeOptions;
use linebender_resource_handle::{Blob, FontData};
use parlance::{FontFeature, FontVariation, Language};

use crate::{
    Analysis, CharInfo, ShapedText,
    itemize::{Item, Segment, TextRange},
    lru_cache::LruCache,
    shape::{
        CharCluster, cache,
        shaped_text::HarfGlyph,
        space::SpaceReach,
        words::{MAX_WORD_CHARS, WordRun, Words},
    },
};

/// Shaping options for one item.
///
/// These are styling options relevant for shaping. They're styling, in that they're not derived
/// from the underlying text. When you [shape the text][`Shaper::shape_text`], you should split the
/// text into [`Item`]s at the points where these options change.
#[derive(Debug)]
pub struct ShapeOptions<'a> {
    /// The font size to shape the item with.
    pub font_size: f32,
    /// The language to shape the item with.
    pub language: Option<Language>,
    /// The font features to shape the item with.
    ///
    /// If there are font features with duplicate [tags][`FontFeature::tag`], later values override
    /// earlier values.
    pub features: &'a [FontFeature],
    /// The font variations that are constant over an item.
    pub variations: &'a [FontVariation],
    /// kun: CSS `font-optical-sizing: auto`: the `opsz` axis at the font size, unless the
    /// variations set it (a font without the axis ignores it).
    pub optical_sizing: bool,
}

/// The font instance to shape an item with.
#[derive(Clone, Debug, PartialEq)]
pub struct FontInstance {
    /// The font.
    pub font: FontData,
    /// Font synthesis suggestions.
    // TODO: Synthesis carries more than we need, and ties us to `fontique`. We can likely change
    // this to opaque user data.
    pub synthesis: fontique::Synthesis,
}

impl FontInstance {
    /// Borrow this font instance.
    #[inline]
    pub fn as_ref(&self) -> FontInstanceRef<'_> {
        FontInstanceRef {
            data: &self.font.data,
            index: self.font.index,
            synthesis: &self.synthesis,
        }
    }
}

/// A borrowed [`FontInstance`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontInstanceRef<'a> {
    /// Blob containing the content of the font file.
    pub data: &'a Blob<u8>,
    /// Index of the font in a collection, or 0 for a single font.
    pub index: u32,
    /// Font synthesis suggestions.
    pub synthesis: &'a fontique::Synthesis,
}

impl From<FontInstanceRef<'_>> for FontInstance {
    #[inline]
    fn from(font: FontInstanceRef<'_>) -> Self {
        Self {
            font: FontData {
                data: font.data.clone(),
                index: font.index,
            },
            synthesis: *font.synthesis,
        }
    }
}

/// Reusable scratch to shape text using [`Self::shape_text`].
pub struct Shaper {
    shape_data_cache: LruCache<cache::ShapeDataKey, harfrust::ShaperData>,
    shape_instance_cache: LruCache<cache::ShapeInstanceId, harfrust::ShaperInstance>,
    shape_plan_cache: LruCache<cache::ShapePlanId, harfrust::ShapePlan>,
    unicode_buffer: Option<harfrust::UnicodeBuffer>,
    features: Vec<harfrust::Feature>,
    /// kun: the item's variations, with `opsz` for optical sizing.
    variations: Vec<FontVariation>,
    char_cluster: CharCluster,
    /// kun: what shaping words gave.
    words: Words,
    word_scratch: WordScratch,
}

impl Default for Shaper {
    fn default() -> Self {
        const MAX_ENTRIES: usize = 16;
        Self {
            shape_data_cache: LruCache::new(MAX_ENTRIES),
            shape_instance_cache: LruCache::new(MAX_ENTRIES),
            shape_plan_cache: LruCache::new(MAX_ENTRIES),
            unicode_buffer: Some(harfrust::UnicodeBuffer::new()),
            features: Vec::new(),
            variations: Vec::new(),
            char_cluster: CharCluster::default(),
            words: Words::default(),
            word_scratch: WordScratch::default(),
        }
    }
}

impl core::fmt::Debug for Shaper {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Shaper").finish_non_exhaustive()
    }
}

impl Shaper {
    /// kun: whether segments are shaped a word at a time where that gives what shaping them
    /// whole does (it does unless turned off), so that a debug build can check it.
    #[doc(hidden)]
    pub fn set_shape_words(&mut self, on: bool) {
        self.words.enabled = on;
    }

    /// kun: how many words were taken from those shaped before, and how many were shaped and
    /// kept, for tests.
    #[doc(hidden)]
    pub fn words_reused_and_shaped(&self) -> (usize, usize) {
        (self.words.reused, self.words.shaped)
    }

    /// kun: how many times two words were shaped again as one, a space and the glyph beside it
    /// shaped together, for tests.
    #[doc(hidden)]
    pub fn words_joined(&self) -> usize {
        self.words.joined
    }

    /// kun: forgets the words shaped: fonts were added, so font selection may pick others. A
    /// [`FontSelector::font_style`] id given out before may be given to another style after.
    #[doc(hidden)]
    pub fn clear_words(&mut self) {
        self.words.clear();
    }

    /// Shape text into glyphs, overwriting `shaped_text`.
    ///
    /// The `text` passed in must be the same as used for producing `analysis`.
    ///
    /// This uses the items returned by `items`, and further itemizes the text into
    /// individually-shapeable segments of constant bidi level and script. `items` should be used to
    /// split on properties like shaping-relevant style changes (e.g., font size) or properties like
    /// language.
    ///
    /// `char_style_indices` holds per-character style indices; these are copied onto
    /// [`Character`][super::Character`] and [`ShapedCluster`][super::ShapedCluster]. A shaped
    /// cluster's style index is that of its logically first constituent character.
    ///
    /// Characters that don't have a particular script have their script resolved based on
    /// surrounding context (see [`Segment::script`]).
    ///
    /// Each segment is then broken into runs of maximal sequences of character clusters for which
    /// `select_font` returns the same font.
    ///
    /// # Panics
    ///
    /// Panics if `items` does not cover the entire source text, or the font returned by
    /// `select_font` is malformed.
    pub fn shape_text<'options>(
        &mut self,
        text: &str,
        analysis: &Analysis,
        // TODO: rename to something like `user_data` (s.t. we don't assume it's a style per se).
        char_style_indices: &[u16],
        items: impl IntoIterator<Item = Item<'options>>,
        mut select_font: impl FontSelector,
        shaped_text: &mut ShapedText,
    ) {
        shaped_text.clear();
        shaped_text.reserve(text.len());

        let char_count = analysis.char_info().len();
        debug_assert_eq!(
            char_style_indices.len(),
            char_count,
            "The number of character style indices must be equal to the character count"
        );

        let mut previous_item_end = 0;
        let mut itemizer = analysis.itemize(text);

        for item in items {
            assert!(
                item.char_end > previous_item_end,
                "item ends must be strictly increasing"
            );
            assert!(
                item.char_end as usize <= char_count,
                "items must not span past the text"
            );

            loop {
                let segment = itemizer
                    .next(
                        #[inline(always)]
                        |text_range| text_range.char_range.end == item.char_end as usize,
                    )
                    .expect("A segment must be yielded, given items tile the full text exactly");

                if shape_segment(
                    self,
                    text,
                    &segment,
                    &item.options,
                    &mut select_font,
                    analysis.char_info(),
                    char_style_indices,
                    shaped_text,
                )
                .is_err()
                {
                    // Abort on error. This happens iff `FontSelector::select_font` failed to return a
                    // font. By aborting we ensure `ShapedText` covers the source text contiguously (as
                    // we need a font to construct `ShapedRun`).
                    return;
                };

                debug_assert!(
                    segment.range.char_range.end <= item.char_end as usize,
                    "Segments must not span past the item."
                );
                if segment.range.char_range.end == item.char_end as usize {
                    break;
                }
            }

            previous_item_end = item.char_end;
        }

        assert_eq!(
            previous_item_end as usize, char_count,
            "`items` does not cover the entire source text"
        );
        debug_assert_eq!(
            shaped_text.characters().len(),
            char_count,
            "All characters must have been processed"
        );
    }
}

/// Shape one segment.
///
/// Returns `Err(())` if shaping should be aborted, which happens iff [`FontSelector::select_font`]
/// returned `None`.
fn shape_segment(
    scx: &mut Shaper,
    text: &str,
    segment: &Segment,
    options: &ShapeOptions<'_>,
    select_font: &mut impl FontSelector,
    char_info: &[CharInfo],
    char_style_indices: &[u16],
    shaped_text: &mut ShapedText,
) -> Result<(), ()> {
    select_font.begin_segment(segment, options);

    let text_range = &segment.range.byte_range;
    let char_range = &segment.range.char_range;

    let segment_text = &text[text_range.clone()];

    // Only process current segment
    let segment_char_info = &char_info[char_range.start..char_range.end];
    let segment_char_style_indices = &char_style_indices[char_range.start..char_range.end];

    if segment_text.is_empty() {
        return Ok(()); // No clusters
    }

    // Shaping inputs that are constant over the segment.
    let direction = if segment.bidi_level.is_rtl() {
        harfrust::Direction::RightToLeft
    } else {
        harfrust::Direction::LeftToRight
    };
    let hb_script = script_to_harfrust(segment.script);
    let language = options
        .language
        .as_ref()
        .and_then(|lang| lang.as_str().parse::<harfrust::Language>().ok());
    scx.features.clear();
    scx.features.extend(options.features.iter().map(|feature| {
        harfrust::Feature::new(
            harfrust::Tag::new(&feature.tag.to_bytes()),
            feature.value as u32,
            ..,
        )
    }));
    // kun: optical sizing sets the `opsz` axis to the font size (CSS Fonts 4), first so that
    // the variations' own would win; a font without the axis ignores it.
    const OPSZ: parlance::Tag = parlance::Tag::new(b"opsz");
    scx.variations.clear();
    if options.optical_sizing && !options.variations.iter().any(|v| v.tag == OPSZ) {
        scx.variations
            .push(FontVariation::new(OPSZ, options.font_size));
    }
    scx.variations.extend_from_slice(options.variations);

    // kun: the shaper's parts, borrowed apart.
    let Shaper {
        shape_data_cache,
        shape_instance_cache,
        shape_plan_cache,
        unicode_buffer,
        features,
        variations,
        char_cluster,
        words,
        word_scratch,
    } = scx;
    let mut harf = Harf {
        shape_data_cache,
        shape_instance_cache,
        shape_plan_cache,
        unicode_buffer,
        features,
        variations,
        direction,
        script: hb_script,
        language,
        font_size: options.font_size,
    };

    // kun: a word at a time, taking the words shaped before, where that gives what shaping the
    // segment whole does.
    if words.may_split(segment.bidi_level.is_ltr(), segment.script, options) {
        let mark = shaped_text.mark();
        if shape_words(
            &mut harf,
            words,
            word_scratch,
            char_cluster,
            text,
            segment,
            options,
            select_font,
            char_info,
            char_style_indices,
            shaped_text,
        )? {
            return Ok(());
        }
        // A font shapes a space together with what is around it: the segment is shaped whole.
        shaped_text.truncate(mark);
        select_font.begin_segment(segment, options);
    }

    // Shape a run of the segment with a single font, appending it to `shaped_text`.
    let mut shape_run = |font: &FontInstance, range: TextRange| {
        harf.shape(
            font,
            text,
            range.byte_range.clone(),
            |glyph_buffer, normalized_coords| {
                shaped_text.push_run(
                    text,
                    range,
                    segment,
                    options,
                    char_info,
                    char_style_indices,
                    font,
                    HarfGlyph::of_buffer(glyph_buffer),
                    normalized_coords,
                );
            },
        );
    };

    // This is the run being accumulated, i.e., consecutive graphemes for which `select_font`
    // returns the same font.
    let mut run: Option<(FontInstance, TextRange)> = None;

    // The segment's characters with their analysis data and style indices.
    let mut chars = segment_text
        .chars()
        .zip(segment_char_info)
        .zip(segment_char_style_indices)
        .map(|((ch, &info), &style_index)| (ch, info, style_index));

    // The lengths of the segment's graphemes in characters.
    let grapheme_lengths = segment_char_info
        .chunk_by(|_, info| !info.is_grapheme_start())
        .map(<[_]>::len);

    // The end of the previous grapheme.
    let mut byte_end = text_range.start;
    let mut char_end = char_range.start;

    for grapheme_length in grapheme_lengths {
        let (byte_start, char_start) = (byte_end, char_end);
        char_cluster.fill(chars.by_ref().take(grapheme_length), &mut byte_end);
        char_end += grapheme_length;
        let grapheme = TextRange {
            byte_range: byte_start..byte_end,
            char_range: char_start..char_end,
        };

        let font = select_font
            .select_font(segment, options, char_cluster)
            .ok_or(())?;

        if let Some((run_font, run_range)) = &mut run
            && run_font.as_ref() == font
        {
            run_range.byte_range.end = byte_end;
            run_range.char_range.end = char_end;
        } else {
            if let Some((run_font, run_range)) = run.take() {
                // The font changed: shape the previous run.
                shape_run(&run_font, run_range);
            }
            run = Some((font.into(), grapheme));
        }
    }
    if let Some((run_font, run_range)) = run {
        shape_run(&run_font, run_range);
    }

    Ok(())
}

/// kun: what shaping a run with harfrust takes, the same over a segment.
struct Harf<'a> {
    shape_data_cache: &'a mut LruCache<cache::ShapeDataKey, harfrust::ShaperData>,
    shape_instance_cache: &'a mut LruCache<cache::ShapeInstanceId, harfrust::ShaperInstance>,
    shape_plan_cache: &'a mut LruCache<cache::ShapePlanId, harfrust::ShapePlan>,
    unicode_buffer: &'a mut Option<harfrust::UnicodeBuffer>,
    features: &'a [harfrust::Feature],
    variations: &'a [FontVariation],
    direction: harfrust::Direction,
    script: harfrust::Script,
    language: Option<harfrust::Language>,
    font_size: f32,
}

impl Harf<'_> {
    /// Shapes `text[byte_range]` with `font`, the text around it as context, and hands harfrust's
    /// glyphs and the font instance's normalized coords to `f`.
    fn shape<R>(
        &mut self,
        font: &FontInstance,
        text: &str,
        byte_range: Range<usize>,
        f: impl FnOnce(&harfrust::GlyphBuffer, &[harfrust::NormalizedCoord]) -> R,
    ) -> R {
        // TODO: How do we want to handle errors like this?
        let font_ref =
            harfrust::FontRef::from_index(font.font.data.as_ref(), font.font.index).unwrap();

        // Create harfrust shaper
        let shaper_data = self.shape_data_cache.entry(
            cache::ShapeDataKey::new(font.font.data.id(), font.font.index),
            || harfrust::ShaperData::new(&font_ref),
        );
        let variations = self.variations;
        let instance = self.shape_instance_cache.entry(
            cache::ShapeInstanceKey::new(
                font.font.data.id(),
                font.font.index,
                &font.synthesis,
                Some(variations),
            ),
            || {
                harfrust::ShaperInstance::from_variations(
                    &font_ref,
                    variations_iter(&font.synthesis, variations),
                )
            },
        );

        let harf_shaper = shaper_data
            .shaper(&font_ref)
            .instance(Some(instance))
            .build();
        let shaper_plan = self.shape_plan_cache.entry(
            cache::ShapePlanKey::new(
                font.font.data.id(),
                font.font.index,
                &font.synthesis,
                self.direction,
                self.script,
                self.language.clone(),
                self.features,
                Some(variations),
            ),
            || {
                harfrust::ShapePlan::new(
                    &harf_shaper,
                    self.direction,
                    Some(self.script),
                    self.language.as_ref(),
                    self.features,
                )
            },
        );

        // Prepare harfrust buffer
        let mut buffer = self.unicode_buffer.take().unwrap();
        buffer.clear();
        buffer.set_cluster_level(harfrust::BufferClusterLevel::MonotoneCharacters);

        // Use the entire run text including newlines.
        let run_text = &text[byte_range.clone()];
        buffer.reserve(run_text.len());
        #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
        for (i, ch) in run_text.chars().enumerate() {
            // Ensure that each cluster's index matches the index into `infos`. This is required
            // for efficient cluster lookup within `data.rs`.
            //
            // In other words, instead of using `buffer.push_str`, which iterates `run_text`
            // with `char_indices`, push each char individually via `.chars` with a cluster index
            // that matches its `infos` counterpart. This allows us to lookup `infos` via cluster
            // index in `data.rs`.
            //
            // kun: a tab shapes as a space, so it draws nothing (fonts seldom
            // map U+0009, and its .notdef box would show); line breaking
            // gives it its advance to the next tab stop, as Chrome draws it.
            let ch = if ch == '\t' { ' ' } else { ch };
            buffer.add(ch, i as u32);
        }

        buffer.set_pre_context(&text[..byte_range.start]);
        buffer.set_post_context(&text[byte_range.end..]);
        buffer.set_direction(self.direction);
        buffer.set_script(self.script);

        if let Some(lang) = &self.language {
            buffer.set_language(lang.clone());
        }

        let glyph_buffer = harf_shaper.shape(
            buffer,
            HarfShapeOptions::new()
                .plan(Some(shaper_plan))
                .features(self.features)
                .point_size(Some(self.font_size)),
        );

        let result = f(&glyph_buffer, harf_shaper.coords());

        // Replace buffer to reuse allocation in the next run.
        *self.unicode_buffer = Some(glyph_buffer.clear());
        result
    }
}

/// kun: scratch for shaping a word at a time.
#[derive(Default)]
struct WordScratch {
    /// The glyphs of the run being put together, clusters counted from its first character.
    run_glyphs: Vec<HarfGlyph>,
    /// The font style of each of a word's characters.
    styles: Vec<u32>,
    /// A word's glyphs, as it is shaped.
    word_glyphs: Vec<HarfGlyph>,
}

/// kun: puts runs of words, each shaped on its own, together into runs of one font, as shaping
/// the text whole makes them; where shaping the text whole takes a space together with the glyph
/// beside it, the words are shaped again as one.
struct WordRuns<'a, 't> {
    text: &'t str,
    segment: &'t Segment,
    options: &'t ShapeOptions<'t>,
    char_info: &'t [CharInfo],
    char_style_indices: &'t [u16],
    shaped_text: &'a mut ShapedText,
    glyphs: &'a mut Vec<HarfGlyph>,
    /// The run being put together: its font, its text and its font instance's coords.
    run: Option<(FontInstance, TextRange, Vec<harfrust::NormalizedCoord>)>,
    /// Where the run's last word starts, words shaped again as one counting as one.
    word: RunWord,
    /// Where the word before it starts, if it is in the run and known.
    previous: Option<RunWord>,
    /// How many times words were shaped again as one.
    joined: usize,
}

/// kun: where a word of a run being put together starts, within the run.
#[derive(Clone, Copy, Default)]
struct RunWord {
    /// Its first glyph, in the run's.
    glyph: usize,
    byte: usize,
    char: usize,
    /// Whether it is a space.
    space: bool,
    /// Whether the word after it is shaped again with it.
    joins_next: bool,
}

/// kun: what a word is shaped again with.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Join {
    Nothing,
    /// The run's last word.
    Last,
    /// The run's last word and the one before it.
    LastTwo,
}

impl WordRuns<'_, '_> {
    /// Appends `range`, shaped on its own with `font` into `glyphs`: all of a word, or its part
    /// in one font, the first if `starts_word`, of a word that is a space if `space` (with other
    /// characters in its grapheme if `space_marks`). `reach` is how far shaping with `font`
    /// reaches across a space (`None`: not known).
    #[expect(clippy::too_many_arguments, reason = "a word's part and what it is in")]
    fn push(
        &mut self,
        harf: &mut Harf<'_>,
        font: &FontInstance,
        range: TextRange,
        glyphs: &[HarfGlyph],
        coords: &[harfrust::NormalizedCoord],
        reach: Option<&SpaceReach>,
        starts_word: bool,
        space: bool,
        space_marks: bool,
    ) {
        let (join, joins_next) = if starts_word
            && self
                .run
                .as_ref()
                .is_some_and(|(run_font, ..)| run_font == font)
        {
            self.joins(
                reach,
                glyphs.first(),
                range.char_range.start,
                space,
                space_marks,
            )
        } else {
            (Join::Nothing, false)
        };
        if let Some((run_font, run_range, _)) = &mut self.run
            && run_font == font
        {
            #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
            let offset = (range.char_range.start - run_range.char_range.start) as u32;
            if join == Join::Nothing {
                if starts_word {
                    self.previous = Some(self.word);
                    self.word = RunWord {
                        glyph: self.glyphs.len(),
                        byte: range.byte_range.start,
                        char: range.char_range.start,
                        space,
                        joins_next,
                    };
                }
                self.glyphs.extend(glyphs.iter().map(|glyph| HarfGlyph {
                    cluster: glyph.cluster + offset,
                    ..*glyph
                }));
            } else {
                // Shape the words from `start` and this one again, as one: from the run's start
                // if the word before the last isn't known.
                let start = if join == Join::LastTwo {
                    let start = self.previous.take().unwrap_or(RunWord {
                        byte: run_range.byte_range.start,
                        char: run_range.char_range.start,
                        ..RunWord::default()
                    });
                    self.word = start;
                    start
                } else {
                    self.word
                };
                #[expect(clippy::cast_possible_truncation, reason = "Deferred")]
                let offset = (start.char - run_range.char_range.start) as u32;
                self.glyphs.truncate(start.glyph);
                let run_glyphs = &mut *self.glyphs;
                harf.shape(
                    font,
                    self.text,
                    start.byte..range.byte_range.end,
                    |buffer, _| {
                        run_glyphs.extend(HarfGlyph::of_buffer(buffer).map(|glyph| HarfGlyph {
                            cluster: glyph.cluster + offset,
                            ..glyph
                        }));
                    },
                );
                self.word.space = space;
                self.word.joins_next = joins_next;
                self.joined += 1;
            }
            run_range.byte_range.end = range.byte_range.end;
            run_range.char_range.end = range.char_range.end;
        } else {
            self.flush();
            self.glyphs.extend_from_slice(glyphs);
            self.previous = None;
            self.word = RunWord {
                glyph: 0,
                byte: range.byte_range.start,
                char: range.char_range.start,
                space,
                // A space with more in its grapheme first in the run: the word after it.
                joins_next: space_marks
                    && matches!(reach, Some(SpaceReach::Beside(beside)) if beside.space_ligatures),
            };
            self.run = Some((font.clone(), range, coords.to_vec()));
        }
    }

    /// What shaping the text whole takes together with `next`, the first glyph of a word starting
    /// at `char` (a space if `space`, with more in its grapheme if `space_marks`), and whether
    /// it takes the word after it too.
    fn joins(
        &self,
        reach: Option<&SpaceReach>,
        next: Option<&HarfGlyph>,
        char: usize,
        space: bool,
        space_marks: bool,
    ) -> (Join, bool) {
        let beside = match reach {
            Some(SpaceReach::Apart) => return (Join::Nothing, false),
            Some(SpaceReach::Beside(beside)) => beside,
            // A font not looked into is taken to reach across.
            _ => return (Join::LastTwo, true),
        };
        let (Some(last), Some(next)) = (self.glyphs.last(), next) else {
            return (Join::LastTwo, true);
        };
        let mut join = if self.word.joins_next {
            Join::Last
        } else {
            Join::Nothing
        };
        let mut joins_next = false;
        if space {
            // The last word's last grapheme is of several characters.
            let marks = char
                .checked_sub(1)
                .is_none_or(|end| !self.char_info[end].is_grapheme_start());
            if beside.moved.before(last.id) {
                join = join.max(Join::Last);
            }
            if beside.substituted.before(last.id)
                || (beside.skips_marks && marks)
                || (beside.space_ligatures && space_marks)
            {
                join = join.max(Join::Last);
                joins_next = true;
            }
        }
        if self.word.space {
            if beside.moved.after(next.id) {
                join = join.max(Join::Last);
            }
            if beside.substituted.after(next.id) {
                join = Join::LastTwo;
            }
        }
        (join, joins_next)
    }

    /// Pushes the run put together onto the shaped text.
    fn flush(&mut self) {
        if let Some((font, range, coords)) = self.run.take() {
            self.shaped_text.push_run(
                self.text,
                range,
                self.segment,
                self.options,
                self.char_info,
                self.char_style_indices,
                &font,
                self.glyphs.iter().copied(),
                &coords,
            );
            self.glyphs.clear();
        }
    }
}

/// kun: shapes `segment` a word at a time, taking the fonts and glyphs kept for a word shaped
/// before with the same styles and options, and keeping those of the others.
///
/// Returns `Ok(false)`, part of the segment shaped, if a font shapes a space together with what
/// is around it further than the glyph beside it, or a character's style has no
/// [`FontSelector::font_style`].
fn shape_words(
    harf: &mut Harf<'_>,
    words: &mut Words,
    scratch: &mut WordScratch,
    char_cluster: &mut CharCluster,
    text: &str,
    segment: &Segment,
    options: &ShapeOptions<'_>,
    select_font: &mut impl FontSelector,
    char_info: &[CharInfo],
    char_style_indices: &[u16],
    shaped_text: &mut ShapedText,
) -> Result<bool, ()> {
    let options_id = words.options_id(segment.script, options, harf.variations);
    scratch.run_glyphs.clear();
    let mut runs = WordRuns {
        text,
        segment,
        options,
        char_info,
        char_style_indices,
        shaped_text,
        glyphs: &mut scratch.run_glyphs,
        run: None,
        word: RunWord::default(),
        previous: None,
        joined: 0,
    };
    let (mut reused, mut shaped) = (0, 0);

    for word in word_ranges(text, char_info, &segment.range) {
        let word_text = &text[word.byte_range.clone()];
        let space = word_text.starts_with(' ');
        let space_marks = space && word.char_range.len() > 1;
        let keep = word.char_range.len() <= MAX_WORD_CHARS;
        if keep {
            scratch.styles.clear();
            for &style_index in &char_style_indices[word.char_range.clone()] {
                let Some(style) = select_font.font_style(style_index) else {
                    return Ok(false);
                };
                scratch.styles.push(style);
            }
            if let Some((word_runs, glyphs)) = words.get(options_id, word_text, &scratch.styles) {
                let (mut byte, mut char) = (word.byte_range.start, word.char_range.start);
                for (index, word_run) in word_runs.iter().enumerate() {
                    let range = TextRange {
                        byte_range: byte..byte + word_run.bytes,
                        char_range: char..char + word_run.chars,
                    };
                    (byte, char) = (range.byte_range.end, range.char_range.end);
                    runs.push(
                        harf,
                        &word_run.font,
                        range,
                        &glyphs[word_run.glyphs.clone()],
                        &word_run.coords,
                        words.known_reach(&word_run.font),
                        index == 0,
                        space,
                        space_marks,
                    );
                }
                reused += 1;
                continue;
            }
        }

        // Pick the fonts grapheme by grapheme, and shape each run of one font on its own.
        scratch.word_glyphs.clear();
        let mut word_runs = Vec::new();
        let mut starts_word = true;
        let mut shape_font_run = |font: FontInstance, range: TextRange| {
            let reach = words.reach(&font);
            if *reach == SpaceReach::Across {
                return false;
            }
            let start = scratch.word_glyphs.len();
            let coords = harf.shape(
                &font,
                text,
                range.byte_range.clone(),
                |glyph_buffer, coords| {
                    scratch
                        .word_glyphs
                        .extend(HarfGlyph::of_buffer(glyph_buffer));
                    Box::<[harfrust::NormalizedCoord]>::from(coords)
                },
            );
            runs.push(
                harf,
                &font,
                range.clone(),
                &scratch.word_glyphs[start..],
                &coords,
                Some(reach),
                starts_word,
                space,
                space_marks,
            );
            starts_word = false;
            word_runs.push(WordRun {
                font,
                bytes: range.byte_range.len(),
                chars: range.char_range.len(),
                glyphs: start..scratch.word_glyphs.len(),
                coords,
            });
            true
        };
        let word_char_info = &char_info[word.char_range.clone()];
        let mut chars = word_text
            .chars()
            .zip(word_char_info)
            .zip(&char_style_indices[word.char_range.clone()])
            .map(|((ch, &info), &style_index)| (ch, info, style_index));
        let grapheme_lengths = word_char_info
            .chunk_by(|_, info| !info.is_grapheme_start())
            .map(<[_]>::len);
        let mut byte_end = word.byte_range.start;
        let mut char_end = word.char_range.start;
        let mut run: Option<(FontInstance, TextRange)> = None;
        for grapheme_length in grapheme_lengths {
            let (byte_start, char_start) = (byte_end, char_end);
            char_cluster.fill(chars.by_ref().take(grapheme_length), &mut byte_end);
            char_end += grapheme_length;
            let grapheme = TextRange {
                byte_range: byte_start..byte_end,
                char_range: char_start..char_end,
            };
            let font = select_font
                .select_font(segment, options, char_cluster)
                .ok_or(())?;
            if let Some((run_font, run_range)) = &mut run
                && run_font.as_ref() == font
            {
                run_range.byte_range.end = byte_end;
                run_range.char_range.end = char_end;
            } else {
                if let Some((run_font, run_range)) = run.take()
                    && !shape_font_run(run_font, run_range)
                {
                    return Ok(false);
                }
                run = Some((font.into(), grapheme));
            }
        }
        if let Some((run_font, run_range)) = run
            && !shape_font_run(run_font, run_range)
        {
            return Ok(false);
        }

        if keep {
            words.insert(
                options_id,
                word_text,
                &scratch.styles,
                word_runs,
                &scratch.word_glyphs,
            );
            shaped += 1;
        }
    }
    runs.flush();

    words.reused += reused;
    words.shaped += shaped;
    words.joined += runs.joined;
    Ok(true)
}

/// kun: the words of the text in `range`: it is split before each space and after it, where a
/// grapheme starts.
fn word_ranges<'a>(
    text: &'a str,
    char_info: &'a [CharInfo],
    range: &TextRange,
) -> impl Iterator<Item = TextRange> + 'a {
    let (byte_start, end) = (range.byte_range.start, range.clone());
    let mut chars = text[range.byte_range.clone()]
        .char_indices()
        .map(move |(offset, ch)| (byte_start + offset, ch))
        .zip(range.char_range.clone());
    let mut start = (range.byte_range.start, range.char_range.start);
    let mut previous = None;
    core::iter::from_fn(move || {
        if start.1 == end.char_range.end {
            return None;
        }
        for ((byte, ch), index) in chars.by_ref() {
            let split = index > start.1
                && char_info[index].is_grapheme_start()
                && (ch == ' ' || previous == Some(' '));
            previous = Some(ch);
            if split {
                let word = TextRange {
                    byte_range: start.0..byte,
                    char_range: start.1..index,
                };
                start = (byte, index);
                return Some(word);
            }
        }
        let word = TextRange {
            byte_range: start.0..end.byte_range.end,
            char_range: start.1..end.char_range.end,
        };
        start = (end.byte_range.end, end.char_range.end);
        Some(word)
    })
}

#[inline]
fn variations_iter<'a>(
    synthesis: &'a fontique::Synthesis,
    item: &'a [FontVariation],
) -> impl Iterator<Item = harfrust::Variation> + 'a {
    synthesis
        .variation_settings()
        .iter()
        .map(|(tag, value)| harfrust::Variation {
            tag: *tag,
            value: *value,
        })
        .chain(item.iter().map(|variation| harfrust::Variation {
            tag: harfrust::Tag::new(&variation.tag.to_bytes()),
            value: variation.value,
        }))
}

pub(crate) fn script_to_harfrust(script: fontique::Script) -> harfrust::Script {
    harfrust::Script::from_iso15924_tag(harfrust::Tag::new(&script.to_bytes()))
        .unwrap_or(harfrust::script::UNKNOWN)
}

/// Implements font selection for shaping.
pub trait FontSelector {
    /// Called when a new segment starts.
    ///
    /// This can be useful to inspect, e.g., the segment's script.
    fn begin_segment(&mut self, segment: &Segment, options: &ShapeOptions<'_>) {
        let _ = (segment, options);
    }

    /// Called once per character cluster within the current segment.
    ///
    /// A character cluster will usually be a grapheme, though if text direction or script changes
    /// mid-grapheme, it will be split over segments.
    ///
    /// Shaping is aborted if this returns `None`; [`ShapedText`] then contains a partial result.
    fn select_font(
        &mut self,
        segment: &Segment,
        options: &ShapeOptions<'_>,
        cluster: &mut CharCluster,
    ) -> Option<FontInstanceRef<'_>>;

    /// kun: an id for what [`Self::select_font`] takes from the style at `style_index`, the same
    /// for styles alike in every text shaped with this [`Shaper`] until
    /// [`Shaper::clear_words`]: with ids, a word shaped before with the same ids, script and
    /// options takes the fonts it took then, without asking for them. `None` shapes the
    /// segment's words anew.
    fn font_style(&self, style_index: u16) -> Option<u32> {
        let _ = style_index;
        None
    }
}
