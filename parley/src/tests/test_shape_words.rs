// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! machopo: tests for text shaped a word at a time, the words kept: text is split before each
//! space and after it, and a word is shaped again only with other fonts asked for (families,
//! width, weight, style) or other shaping options. Only where shaping a word on its own gives
//! what shaping the text whole does: left to right, and with fonts whose rules take a space with
//! the glyph beside it at most (`test_shape_words_space` makes fonts with one rule or two). A
//! debug build also shapes every text shaped a word at a time whole, and compares the two.

use alloc::borrow::{Cow, ToOwned};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use fontique::{Blob, Collection, CollectionOptions, SourceCache};
use parlance::Language;
use write_fonts::tables::layout::LookupFlag;

use super::test_shape_words_space::{Rules, extra, font, font_named, g, ligature};
use crate::{
    FontContext, FontFamily, FontFamilyName, FontFeatures, FontVariations, FontWeight, Layout,
    LayoutContext, StyleProperty,
};

pub(super) type Contexts = (FontContext, LayoutContext<u32>);

/// A font context with two fonts made here only: `Made`, without rules, and `Reaching`, with a
/// ligature that skips base glyphs, a space too.
fn contexts() -> Contexts {
    let reaching = Rules {
        gsub: alloc::vec![ligature(
            &[g('a'), g('b')],
            extra(0),
            LookupFlag::IGNORE_BASE_GLYPHS,
        )],
        ..Default::default()
    };
    contexts_with(&[font(Rules::default()), font_named("Reaching", reaching)])
}

/// A font context with the fonts in `files` only.
pub(super) fn contexts_with(files: &[Vec<u8>]) -> Contexts {
    let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    for file in files {
        collection.register_fonts(Blob::new(Arc::new(file.clone())), None);
    }
    let fonts = FontContext {
        collection,
        source_cache: SourceCache::default(),
    };
    (fonts, LayoutContext::new())
}

type Styles<'a> = &'a [(Range<usize>, StyleProperty<'static, u32>)];

/// `text` in 20px `family`, with `styles` over parts of it.
pub(super) fn build(
    fonts: &mut FontContext,
    cx: &mut LayoutContext<u32>,
    family: &str,
    text: &str,
    styles: Styles<'_>,
) -> Layout<u32> {
    let mut builder = cx.ranged_builder(fonts, text, 1.0, true);
    builder.push_default(StyleProperty::FontFamily(FontFamily::Single(
        FontFamilyName::named(family),
    )));
    builder.push_default(StyleProperty::FontSize(20.0));
    for (range, style) in styles {
        builder.push(style.clone(), range.clone());
    }
    builder.build(text)
}

/// How many words laying out `text` took from those shaped before, and how many it shaped and
/// kept.
fn words(cx: &mut Contexts, family: &str, text: &str, styles: Styles<'_>) -> (usize, usize) {
    let (reused, shaped) = cx.1.words_reused_and_shaped();
    build(&mut cx.0, &mut cx.1, family, text, styles);
    let (now_reused, now_shaped) = cx.1.words_reused_and_shaped();
    (now_reused - reused, now_shaped - shaped)
}

/// `words` of `text` in `Made`, all of it with `style`.
fn styled(cx: &mut Contexts, text: &str, style: StyleProperty<'static, u32>) -> (usize, usize) {
    words(cx, "Made", text, &[(0..text.len(), style)])
}

#[test]
fn text_is_split_before_and_after_each_space() {
    // `aa`, ` ` and `bb` are shaped; the rest are taken.
    assert_eq!(
        words(&mut contexts(), "Made", "aa bb aa bb aa", &[]),
        (6, 3)
    );
    // `aa`, ` `, ` `, `bb`.
    assert_eq!(words(&mut contexts(), "Made", "aa  bb", &[]), (1, 3));
    // A space's combining mark stays with it: `aa`, ` \u{301}bb`.
    assert_eq!(words(&mut contexts(), "Made", "aa \u{301}bb", &[]), (0, 2));
}

#[test]
fn a_long_word_is_not_kept() {
    let a32 = "a".repeat(32);
    let a33 = "a".repeat(33);
    assert_eq!(
        words(&mut contexts(), "Made", &format!("{a32} {a32}"), &[]),
        (1, 2)
    );
    // Only the space is kept.
    assert_eq!(
        words(&mut contexts(), "Made", &format!("{a33} {a33}"), &[]),
        (0, 1)
    );
}

#[test]
fn a_word_asking_for_other_fonts_or_options_is_shaped_again() {
    let bold = StyleProperty::FontWeight(FontWeight::BOLD);
    assert_eq!(
        words(&mut contexts(), "Made", "aa aa", &[(3..5, bold)]),
        (0, 3)
    );
    // A brush asks for the same fonts.
    let brush = StyleProperty::Brush(1);
    assert_eq!(
        words(&mut contexts(), "Made", "aa aa", &[(3..5, brush)]),
        (1, 2)
    );
    let mut cx = contexts();
    assert_eq!(words(&mut cx, "Made", "aa", &[]), (0, 1));
    let larger = StyleProperty::FontSize(30.0);
    assert_eq!(styled(&mut cx, "aa", larger), (0, 1));
    let no_ligatures =
        StyleProperty::FontFeatures(FontFeatures::Source(Cow::Borrowed("\"liga\" 0")));
    assert_eq!(styled(&mut cx, "aa", no_ligatures), (0, 1));
    let weight_axis =
        StyleProperty::FontVariations(FontVariations::Source(Cow::Borrowed("\"wght\" 500")));
    assert_eq!(styled(&mut cx, "aa", weight_axis), (0, 1));
    let turkish = StyleProperty::Locale(Some(Language::parse("tr").unwrap()));
    assert_eq!(styled(&mut cx, "aa", turkish), (0, 1));
}

#[test]
fn a_word_in_another_script_is_shaped_again() {
    // The font has neither digits nor Greek: font selection takes it anyway, the first family.
    let mut cx = contexts_with(&[font(Rules::default())]);
    assert_eq!(words(&mut cx, "Made", "ab 12", &[]), (0, 3));
    // ` 12` takes the script of the Greek before it.
    assert_eq!(words(&mut cx, "Made", "\u{3b1}\u{3b2} 12", &[]), (0, 3));
}

/// Distinct words: `0` to `count - 1`.
fn numbers(count: usize) -> String {
    (0..count)
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn the_words_kept_start_again_once_there_are_too_many() {
    let mut cx = contexts();
    // 2^15 words with the space.
    words(&mut cx, "Made", &numbers((1 << 15) - 1), &[]);
    assert_eq!(words(&mut cx, "Made", "0", &[]), (1, 0));
    // One more: the cache starts again with it.
    assert_eq!(words(&mut cx, "Made", "x", &[]), (0, 1));
    assert_eq!(words(&mut cx, "Made", "0 0", &[]), (1, 2));
}

#[test]
fn the_words_kept_start_again_with_the_ids_of_options() {
    let mut cx = contexts();
    let size = |i: u16| StyleProperty::FontSize(f32::from(i) + 1.0);
    // 2^12 sets of options take every id.
    for i in 0..1 << 12 {
        assert_eq!(styled(&mut cx, "aa", size(i)), (0, 1));
    }
    // Another takes one given out before: no word kept with it is taken.
    assert_eq!(styled(&mut cx, "aa", size(1 << 12)), (0, 1));
    // The first takes a new one.
    assert_eq!(styled(&mut cx, "aa aa", size(0)), (1, 2));
}

#[test]
fn the_words_kept_start_again_with_the_ids_of_font_styles() {
    let mut cx = contexts();
    let weight = |i: u16| StyleProperty::FontWeight(FontWeight::new(1.0 + f32::from(i) / 8.0));
    // 2^12 font styles take every id.
    for i in 0..1 << 12 {
        assert_eq!(styled(&mut cx, "aa", weight(i)), (0, 1));
    }
    // Another (bold enough to embolden) takes one given out before: no word kept with it is
    // taken.
    assert_eq!(styled(&mut cx, "aa", weight(1 << 12)), (0, 1));
    // The first takes a new one.
    assert_eq!(styled(&mut cx, "aa aa", weight(0)), (1, 2));
}

#[test]
fn text_is_shaped_whole_where_words_would_shape_otherwise() {
    // `Reaching` shapes what is on the two sides of a space together.
    assert_eq!(words(&mut contexts(), "Reaching", "aa bb aa", &[]), (0, 0));
    // So does a text with a word in it.
    let reaching =
        || StyleProperty::FontFamily(FontFamily::Single(FontFamilyName::named("Reaching")));
    assert_eq!(
        words(&mut contexts(), "Made", "aa bb aa", &[(3..5, reaching())]),
        (0, 0)
    );
    // A feature that is off unless asked for: what a font does with it isn't looked into.
    let small_caps = StyleProperty::FontFeatures(FontFeatures::Source(Cow::Borrowed("\"smcp\"")));
    assert_eq!(
        words(&mut contexts(), "Made", "aa bb aa", &[(0..8, small_caps)]),
        (0, 0)
    );
    // Right to left: the words of a run go the other way.
    assert_eq!(
        words(
            &mut contexts(),
            "Made",
            "\u{5d0}\u{5d1} \u{5d0}\u{5d1}",
            &[]
        ),
        (0, 0)
    );
    // Arabic, even left to right: harfrust's Arabic shaper looks at the text around a run. (The
    // font has no Arabic: font selection takes it anyway, the first family.)
    assert_eq!(
        words(
            &mut contexts_with(&[font(Rules::default())]),
            "Made",
            "\u{202d}\u{628}\u{628} \u{628}\u{628}\u{202c}",
            &[]
        ),
        (0, 0)
    );
    // A font met after runs of others were put together: the segment is shaped whole, without
    // them.
    let bold = StyleProperty::FontWeight(FontWeight::BOLD);
    assert_eq!(
        words(
            &mut contexts(),
            "Made",
            "aa bb aa",
            &[(3..5, bold), (6..8, reaching())]
        ),
        (0, 0)
    );
}

#[test]
fn words_are_shaped_again_once_cleared() {
    let mut cx = contexts();
    assert_eq!(words(&mut cx, "Made", "aa", &[]), (0, 1));
    cx.1.clear_shapings();
    assert_eq!(words(&mut cx, "Made", "aa bb", &[]), (0, 3));
}

/// Text in many scripts, so that fonts fall back inside words, with repeated words.
#[cfg(feature = "system")]
const TEXTS: &[&str] = &[
    "The quick brown fox jumps over the lazy dog. AV To Wa \u{201c}Yes,\u{201d} 1/2 fi fl ffi -- a  b\tc. The fox, the dog: the end.",
    "\u{41f}\u{440}\u{438}\u{432}\u{435}\u{442} \u{43c}\u{438}\u{440} \u{41f}\u{440}\u{438}\u{432}\u{435}\u{442}. \u{393}\u{3b5}\u{3b9}\u{3ac} \u{3c3}\u{3bf}\u{3c5}. \u{4f60}\u{597d}\u{ff0c}\u{4e16}\u{754c} \u{4f60}\u{597d}. \u{3053}\u{3093}\u{306b}\u{3061}\u{306f} \u{4e16}\u{754c}. \u{c548}\u{b155} \u{c138}\u{c0c1} \u{c548}\u{b155}.",
    "\u{928}\u{92e}\u{938}\u{94d}\u{924}\u{947} \u{926}\u{941}\u{928}\u{93f}\u{92f}\u{93e} \u{928}\u{92e}\u{938}\u{94d}\u{924}\u{947}. \u{e2a}\u{e27}\u{e31}\u{e2a}\u{e14}\u{e35} \u{e0a}\u{e32}\u{e27}\u{e42}\u{e25}\u{e01}. \u{1f600} \u{1f44d}\u{1f3fd} a\u{1f600}b \u{1f600}",
    "\u{5e9}\u{5dc}\u{5d5}\u{5dd} hello \u{5e2}\u{5d5}\u{5dc}\u{5dd}. \u{645}\u{631}\u{62d}\u{628}\u{627} hello world. a\u{301} e\u{301}e x\u{20dd} a\u{200d}b a\u{200b}b \u{301}a",
];

/// With each of the system's font families, text shaped a word at a time is what shaping it
/// whole gives (in a release build too).
#[cfg(feature = "system")]
#[test]
fn system_fonts_shape_words_as_whole_text() {
    let mut fonts = FontContext::new();
    let families: Vec<String> = fonts.collection.family_names().map(str::to_owned).collect();
    let mut words = LayoutContext::new();
    let mut whole = LayoutContext::new();
    whole.set_shape_words(false);
    for family in &families {
        for text in TEXTS {
            let by_words = build(&mut fonts, &mut words, family, text, &[]);
            let by_whole = build(&mut fonts, &mut whole, family, text, &[]);
            assert!(
                **by_words.shaped_text() == **by_whole.shaped_text(),
                "{family}: {text}"
            );
        }
    }
}
