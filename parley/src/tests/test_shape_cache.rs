// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! machopo: tests for the shapings kept: a layout's, taken by the next layout of the same text
//! styled alike in what changes shaping, and a word's. A debug build also shapes every text that
//! took a shaping again, whole, and compares the two.

use alloc::sync::Arc;
use alloc::vec::Vec;

use peniko::color::palette;

use super::utils::ColorBrush;
use super::utils::fonts::{FONT_FAMILY_LIST, create_font_context};
use crate::{
    FontContext, FontFamily, FontWeight, InlineBox, InlineBoxKind, Layout, LayoutContext,
    LineHeight, PositionedLayoutItem, TextStyle, VerticalAlign,
};

fn style(font_size: f32) -> TextStyle<'static, 'static, ColorBrush> {
    TextStyle {
        font_family: FontFamily::from(FONT_FAMILY_LIST),
        font_size,
        line_height: LineHeight::Absolute(30.),
        ..TextStyle::default()
    }
}

/// A piece of a paragraph: text, text of another color, bold text, an inline object with a width,
/// or an out-of-flow box.
#[derive(Clone, Copy)]
enum Piece {
    Text(&'static str),
    Green(&'static str),
    Bold(&'static str),
    Object(f32),
    OutOfFlow,
}
use Piece::{Bold, Green, Object, OutOfFlow, Text};

struct Contexts {
    fcx: FontContext,
    lcx: LayoutContext<ColorBrush>,
}

impl Contexts {
    fn new() -> Self {
        Self {
            fcx: create_font_context(),
            lcx: LayoutContext::new(),
        }
    }

    fn build_scaled(&mut self, pieces: &[Piece], font_size: f32, scale: f32) -> Layout<ColorBrush> {
        let style = style(font_size);
        let mut builder = self.lcx.tree_builder(&mut self.fcx, scale, false, &style);
        for (id, piece) in pieces.iter().enumerate() {
            let inline_box = |kind, width| InlineBox {
                id: id as u64,
                kind,
                index: 0,
                width,
                height: 10.,
                baseline: None,
                vertical_align: VerticalAlign::BASELINE,
            };
            match *piece {
                Text(text) => builder.push_text(text),
                Green(text) => {
                    builder.push_style_span(TextStyle {
                        brush: ColorBrush::new(palette::css::GREEN),
                        ..style.clone()
                    });
                    builder.push_text(text);
                    builder.pop_style_span();
                }
                Bold(text) => {
                    builder.push_style_span(TextStyle {
                        font_weight: FontWeight::BOLD,
                        ..style.clone()
                    });
                    builder.push_text(text);
                    builder.pop_style_span();
                }
                Object(width) => builder.push_inline_box(inline_box(InlineBoxKind::InFlow, width)),
                OutOfFlow => builder.push_inline_box(inline_box(InlineBoxKind::OutOfFlow, 0.)),
            }
        }
        let mut layout = Layout::default();
        builder.build_into(&mut layout);
        layout.break_all_lines(None);
        layout
    }

    fn build(&mut self, pieces: &[Piece]) -> Layout<ColorBrush> {
        self.build_scaled(pieces, 20., 1.)
    }

    /// Builds `pieces`; whether that took the shaping of a layout before.
    fn reuses(&mut self, pieces: &[Piece]) -> bool {
        let before = self.lcx.shapings_reused();
        self.build(pieces);
        self.lcx.shapings_reused() - before == 1
    }

    /// How many words building `pieces` took from those shaped before, and how many it shaped
    /// and kept.
    fn words(&mut self, pieces: &[Piece]) -> (usize, usize) {
        self.words_scaled(pieces, 20., 1.)
    }

    fn words_scaled(&mut self, pieces: &[Piece], font_size: f32, scale: f32) -> (usize, usize) {
        let (reused, shaped) = self.lcx.words_reused_and_shaped();
        self.build_scaled(pieces, font_size, scale);
        let (now_reused, now_shaped) = self.lcx.words_reused_and_shaped();
        (now_reused - reused, now_shaped - shaped)
    }
}

/// The glyphs of a layout's lines (id and advance) and the widths of its boxes, in order.
fn items(layout: &Layout<ColorBrush>) -> Vec<(u32, f32)> {
    layout
        .lines()
        .flat_map(|line| line.items())
        .flat_map(|item| match item {
            PositionedLayoutItem::GlyphRun(glyph_run) => glyph_run
                .glyphs()
                .map(|glyph| (glyph.id, glyph.advance))
                .collect::<Vec<_>>(),
            PositionedLayoutItem::InlineBox(inline_box) => {
                alloc::vec![(u32::MAX, inline_box.width)]
            }
        })
        .collect()
}

#[test]
fn a_layout_alike_takes_the_shaping_of_the_first() {
    let mut cx = Contexts::new();
    let pieces = [Text("aaa bb ccc")];
    let first = cx.build(&pieces);
    assert_eq!(cx.lcx.shapings_reused(), 0);
    let second = cx.build(&pieces);
    assert_eq!(cx.lcx.shapings_reused(), 1);
    assert!(Arc::ptr_eq(first.shaped_text(), second.shaped_text()));
    assert_eq!(items(&first), items(&second));

    // Another text is shaped.
    assert!(!cx.reuses(&[Text("aaa bb ccd")]));
    assert!(cx.reuses(&[Text("aaa bb ccd")]));
}

#[test]
fn a_color_is_not_what_changes_shaping() {
    let mut cx = Contexts::new();
    let plain = cx.build(&[Text("aaa bbb")]);
    let before = cx.lcx.shapings_reused();
    let colored = cx.build(&[Text("aaa "), Green("bbb")]);
    assert_eq!(cx.lcx.shapings_reused() - before, 1);
    assert_eq!(items(&plain), items(&colored));

    // Each character takes its own layout's style.
    let brushes = |layout: &Layout<ColorBrush>| -> Vec<ColorBrush> {
        layout
            .lines()
            .flat_map(|line| line.items())
            .filter_map(|item| match item {
                PositionedLayoutItem::GlyphRun(glyph_run) => Some(glyph_run.style().brush),
                PositionedLayoutItem::InlineBox(_) => None,
            })
            .collect()
    };
    assert_eq!(brushes(&plain), [ColorBrush::default()]);
    assert_eq!(
        brushes(&colored),
        [ColorBrush::default(), ColorBrush::new(palette::css::GREEN)]
    );
}

#[test]
fn a_font_size_or_scale_changes_shaping() {
    let mut cx = Contexts::new();
    let pieces = [Text("aaa bbb")];
    cx.build_scaled(&pieces, 20., 1.);
    let before = cx.lcx.shapings_reused();
    cx.build_scaled(&pieces, 30., 1.);
    // The same font size once scaled, of another scale.
    cx.build_scaled(&pieces, 10., 2.);
    assert_eq!(cx.lcx.shapings_reused(), before);
    cx.build_scaled(&pieces, 10., 2.);
    assert_eq!(cx.lcx.shapings_reused(), before + 1);
}

#[test]
fn a_font_weight_changes_shaping() {
    let mut cx = Contexts::new();
    cx.build(&[Text("aaa bbb")]);
    assert!(!cx.reuses(&[Text("aaa "), Bold("bbb")]));
    // Bold elsewhere is another shaping too.
    assert!(!cx.reuses(&[Text("aaa"), Bold(" bbb")]));
    assert!(cx.reuses(&[Text("aaa "), Bold("bbb")]));
    // All of it bold: styled apart at the same places as the first.
    assert!(!cx.reuses(&[Bold("aaa bbb")]));
}

#[test]
fn an_out_of_flow_box_splits_shaping_where_it_is() {
    let mut cx = Contexts::new();
    assert!(!cx.reuses(&[Text("aaa"), OutOfFlow, Text("bbb")]));
    assert!(cx.reuses(&[Text("aaa"), OutOfFlow, Text("bbb")]));
    // The same text with the box elsewhere, and without it.
    assert!(!cx.reuses(&[Text("aa"), OutOfFlow, Text("abbb")]));
    assert!(!cx.reuses(&[Text("aaabbb")]));
}

#[test]
fn an_inline_object_is_a_character_of_the_text() {
    let mut cx = Contexts::new();
    let narrow = [Text("aaa"), Object(10.), Text("bbb")];
    let wide = [Text("aaa"), Object(40.), Text("bbb")];
    let first = cx.build(&narrow);
    // Its width isn't what changes shaping, and each layout has its own.
    let before = cx.lcx.shapings_reused();
    let second = cx.build(&wide);
    let third = cx.build(&narrow);
    assert_eq!(cx.lcx.shapings_reused() - before, 2);
    assert!(items(&first).contains(&(u32::MAX, 10.)));
    assert!(items(&second).contains(&(u32::MAX, 40.)));
    assert_eq!(items(&first), items(&third));
    assert_eq!(second.full_width() - first.full_width(), 30.);

    // Elsewhere in the text, it is another text.
    assert!(!cx.reuses(&[Text("aa"), Object(10.), Text("abbb")]));
}

#[test]
fn a_word_is_shaped_once() {
    let mut cx = Contexts::new();
    // `aa`, ` ` and `bb` are shaped; the rest are taken.
    assert_eq!(cx.words(&[Text("aa bb aa bb aa")]), (6, 3));
    // Another text takes the words shaped before.
    assert_eq!(cx.words(&[Text("bb aa cc")]), (4, 1));
    // A layout that takes the shaping of one alike shapes no words.
    assert_eq!(cx.words(&[Text("bb aa cc")]), (0, 0));
}

#[test]
fn a_word_of_other_fonts_or_another_size_is_shaped_again() {
    let mut cx = Contexts::new();
    // `aa`, ` ` and a bold `aa`.
    assert_eq!(cx.words(&[Text("aa "), Bold("aa")]), (0, 3));
    // A color asks for the same fonts.
    assert_eq!(cx.words(&[Text("bb "), Green("bb")]), (2, 1));
    // Another font size, and the same font size once scaled, of another scale.
    assert_eq!(cx.words_scaled(&[Text("aa")], 30., 1.), (0, 1));
    assert_eq!(cx.words_scaled(&[Text("aa")], 10., 2.), (0, 1));
    assert_eq!(cx.words_scaled(&[Text("aa aa")], 10., 2.), (2, 1));
    // The same font size before it is scaled.
    assert_eq!(cx.words_scaled(&[Text("aa")], 20., 2.), (0, 1));
}

#[test]
fn an_inline_object_is_between_words_and_not_kept() {
    let mut cx = Contexts::new();
    // `aa` and ` ` are shaped; the `aa` after the object and the one after the space are taken.
    assert_eq!(cx.words(&[Text("aa"), Object(10.), Text("aa aa")]), (2, 2));
    // Objects alone, and one first in the text.
    assert_eq!(cx.words(&[Object(10.), Object(20.)]), (0, 0));
    let layout = cx.build(&[Object(10.), Text("aa bb")]);
    assert_eq!(cx.words(&[Text("aa bb cc")]), (4, 1));

    // What shaping the text whole gives.
    let mut whole = Contexts::new();
    whole.lcx.set_shape_words(false);
    let expected = whole.build(&[Object(10.), Text("aa bb")]);
    assert_eq!(whole.lcx.words_reused_and_shaped(), (0, 0));
    assert_eq!(items(&layout), items(&expected));
}

#[test]
fn a_default_ignorable_after_a_space_is_shaped_with_it() {
    let mut cx = Contexts::new();
    // A soft hyphen first in a word: shaping the text whole takes it with the space before it
    // where harfrust positions marks itself.
    let pieces = [Text("aa \u{ad}bb aa")];
    let layout = cx.build(&pieces);
    assert_eq!(cx.lcx.words_joined(), 1);
    // One last in a word is shaped with that word alone.
    cx.build(&[Text("aa bb\u{ad} aa")]);
    assert_eq!(cx.lcx.words_joined(), 1);

    let mut whole = Contexts::new();
    whole.lcx.set_shape_words(false);
    assert_eq!(items(&layout), items(&whole.build(&pieces)));

    // The same in a font whose space is beside nothing it changes.
    let mut cx = Contexts::new();
    let style = TextStyle {
        font_family: FontFamily::named("Noto Sans CJK SC"),
        ..style(20.)
    };
    let mut builder = cx.lcx.tree_builder(&mut cx.fcx, 1., false, &style);
    builder.push_text("aa \u{ad}bb aa");
    builder.build_into(&mut Layout::default());
    assert_eq!(cx.lcx.words_reused_and_shaped(), (2, 3));
    assert_eq!(cx.lcx.words_joined(), 1);
}
