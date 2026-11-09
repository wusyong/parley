// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! machopo: tests for a layout's shared shaped text ([`Layout::shaped_text`]) and the glyph runs
//! kept without their layout ([`GlyphSpan`]).

use alloc::sync::Arc;
use alloc::vec::Vec;

use peniko::color::palette;

use super::utils::ColorBrush;
use super::utils::fonts::{FONT_FAMILY_LIST, create_font_context};
use crate::{
    Alignment, AlignmentOptions, FontFamily, Glyph, GlyphSpan, InlineBox, InlineBoxKind, Layout,
    LayoutContext, LineHeight, PositionedLayoutItem, ShapedText, SpanCluster, TabStops, TextStyle,
    VerticalAlign,
};

fn style() -> TextStyle<'static, 'static, ColorBrush> {
    TextStyle {
        font_family: FontFamily::from(FONT_FAMILY_LIST),
        font_size: 20.,
        line_height: LineHeight::Absolute(30.),
        letter_spacing: 1.5,
        word_spacing: 2.5,
        ..TextStyle::default()
    }
}

/// A piece of a paragraph: text, text of another style, or an inline object with a width.
#[derive(Clone, Copy)]
enum Piece {
    Text(&'static str),
    Other(&'static str),
    Object(f32),
}
use Piece::{Object, Other, Text};

fn build_into(
    layout: &mut Layout<ColorBrush>,
    pieces: &[Piece],
    style: &TextStyle<'_, '_, ColorBrush>,
) {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut builder = lcx.tree_builder(&mut fcx, 1., false, style);
    for (id, piece) in pieces.iter().enumerate() {
        match *piece {
            Text(text) => builder.push_text(text),
            Other(text) => {
                builder.push_style_span(TextStyle {
                    brush: ColorBrush::new(palette::css::GREEN),
                    ..style.clone()
                });
                builder.push_text(text);
                builder.pop_style_span();
            }
            Object(width) => builder.push_inline_box(InlineBox {
                id: id as u64,
                kind: InlineBoxKind::InFlow,
                index: 0,
                width,
                height: 10.,
                baseline: None,
                vertical_align: VerticalAlign::BASELINE,
            }),
        }
    }
    builder.build_into(layout);
}

fn build(pieces: &[Piece]) -> Layout<ColorBrush> {
    let mut layout = Layout::default();
    build_into(&mut layout, pieces, &style());
    layout
}

/// What is kept of a glyph run, and what its layout gives for it: its glyphs, and the clusters of
/// its style in its run on the line.
struct Kept {
    span: GlyphSpan,
    glyphs: Vec<Glyph>,
    clusters: Vec<SpanCluster>,
}

fn glyph_runs(layout: &Layout<ColorBrush>) -> Vec<Kept> {
    layout
        .lines()
        .flat_map(|line| line.items())
        .filter_map(|item| match item {
            PositionedLayoutItem::GlyphRun(glyph_run) => Some(Kept {
                span: glyph_run.span(),
                glyphs: glyph_run.glyphs().collect(),
                clusters: glyph_run
                    .run()
                    .visual_clusters()
                    .filter(|cluster| cluster.style_index() == glyph_run.style_index())
                    .map(|cluster| SpanCluster {
                        text_range: cluster.text_range(),
                        advance: cluster.advance(),
                    })
                    .collect(),
            }),
            PositionedLayoutItem::InlineBox(_) => None,
        })
        .collect()
}

/// Checks that each of `kept`'s spans gives with `shaped_text` what its layout gave.
fn assert_spans(kept: &[Kept], shaped_text: &ShapedText) {
    for Kept {
        span,
        glyphs,
        clusters,
    } in kept
    {
        assert!(!glyphs.is_empty());
        assert_eq!(&span.glyphs(shaped_text).collect::<Vec<_>>(), glyphs);
        assert_eq!(&span.clusters(shaped_text).collect::<Vec<_>>(), clusters);
        let start = clusters[0].text_range.start;
        let end = clusters.iter().map(|c| c.text_range.end).max().unwrap();
        assert_eq!(span.text_range(shaped_text), Some((start, end)));
    }
}

/// Lines of several glyph runs, justified, with letter and word spacing.
fn justified(pieces: &[Piece], max: f32) -> Layout<ColorBrush> {
    let mut layout = build(pieces);
    layout.set_alignment(Alignment::Justify, AlignmentOptions::default());
    layout.break_all_lines(Some(max));
    layout
}

#[test]
fn a_span_gives_what_its_glyph_run_does() {
    let pieces = [
        Text("aaa bb"),
        Other("b ccc dd"),
        Text("d eee fff ggg "),
        Other("hhh"),
        // The last grapheme is two characters.
        Text(" iie\u{301}"),
    ];
    let layout = justified(&pieces, 130.);
    assert!(layout.len() > 2);
    let kept = glyph_runs(&layout);
    // Glyph runs that are part of their run on the line, and lines with more than one.
    assert!(kept.len() > layout.len() + 2);
    assert_spans(&kept, layout.shaped_text());

    // Each has its own spacing.
    let spaced: Vec<f32> = kept[0].glyphs.iter().map(|glyph| glyph.advance).collect();
    let mut unspaced = build(&pieces);
    unspaced.break_all_lines(Some(130.));
    let unspaced: Vec<f32> = glyph_runs(&unspaced)[0]
        .glyphs
        .iter()
        .map(|glyph| glyph.advance)
        .collect();
    assert_eq!(spaced.len(), unspaced.len());
    assert_ne!(spaced, unspaced);
}

#[test]
fn a_span_gives_what_its_right_to_left_glyph_run_does() {
    // Arabic letters in words of three, some of another style, among Latin ones.
    let pieces = [
        Text("\u{628}\u{628}\u{628} \u{62A}\u{62A}"),
        Other("\u{62A} \u{628}\u{62A}\u{628} abc \u{62A}"),
        Text("\u{628}\u{62A} \u{628}\u{628}\u{62A}"),
    ];
    let layout = justified(&pieces, 90.);
    assert!(layout.len() > 1);
    let kept = glyph_runs(&layout);
    assert!(kept.len() > layout.len() + 1);
    assert_spans(&kept, layout.shaped_text());

    // The first cluster is the visually first one: the logically last.
    assert!(kept.iter().any(|kept| {
        let clusters = &kept.clusters;
        clusters.len() > 1 && clusters[0].text_range.start > clusters[1].text_range.start
    }));
}

#[test]
fn the_shaped_text_that_is_kept_outlives_the_layout() {
    let mut layout = justified(&[Text("aaa bb"), Other("b ccc"), Text(" ddd")], 80.);
    let kept = glyph_runs(&layout);
    let shaped_text = Arc::clone(layout.shaped_text());

    // The layout is built again: it has another shaped text.
    build_into(&mut layout, &[Text("xyz")], &style());
    layout.break_all_lines(None);
    assert!(!Arc::ptr_eq(&shaped_text, layout.shaped_text()));
    assert_eq!(layout.shaped_text().shaped_clusters().len(), 3);
    assert_spans(&kept, &shaped_text);
}

#[test]
fn a_layout_built_again_uses_the_shaped_text_no_one_else_has() {
    let mut layout = build(&[Text("aaa bbb")]);
    let before = Arc::as_ptr(layout.shaped_text());
    build_into(&mut layout, &[Text("xyz")], &style());
    assert_eq!(Arc::as_ptr(layout.shaped_text()), before);
    assert_eq!(layout.shaped_text().shaped_clusters().len(), 3);
}

#[test]
fn breaking_lines_again_leaves_the_shaped_text_as_it_is() {
    let mut layout = build(&[Text("aaa bbb ccc"), Object(10.), Text("ddd")]);
    let shaped_text = Arc::clone(layout.shaped_text());
    layout.break_all_lines(Some(60.));
    layout.break_all_lines(Some(200.));
    assert!(Arc::ptr_eq(&shaped_text, layout.shaped_text()));
}

fn advances(shaped_text: &ShapedText) -> Vec<f32> {
    shaped_text
        .shaped_clusters()
        .iter()
        .map(|cluster| cluster.advance)
        .collect()
}

#[test]
fn a_tab_stop_changes_a_copy_of_shaped_text_that_is_shared() {
    let tabbed = TextStyle {
        tab_stops: Some(TabStops {
            interval: 100.,
            min_advance: 0.,
        }),
        ..style()
    };
    let mut layout = Layout::default();
    build_into(&mut layout, &[Text("a\tb")], &tabbed);

    // No one else has it: it is changed in place.
    let shaped = advances(layout.shaped_text());
    let before = Arc::as_ptr(layout.shaped_text());
    layout.break_all_lines(None);
    assert_eq!(Arc::as_ptr(layout.shaped_text()), before);
    let laid_out = advances(layout.shaped_text());
    assert_ne!(laid_out, shaped);

    build_into(&mut layout, &[Text("a\tb")], &tabbed);
    let shaped_text = Arc::clone(layout.shaped_text());
    layout.break_all_lines(None);
    assert!(!Arc::ptr_eq(&shaped_text, layout.shaped_text()));
    assert_eq!(advances(&shaped_text), shaped);
    assert_eq!(advances(layout.shaped_text()), laid_out);
}

#[test]
fn an_inline_object_changes_a_copy_of_shaped_text_that_is_shared() {
    let mut layout = build(&[Text("a"), Object(10.), Text("b")]);
    let shaped_text = Arc::clone(layout.shaped_text());
    let shaped = advances(&shaped_text);
    assert_eq!(shaped[1], 10.);

    // The width it has leaves the shaped text as it is.
    layout.edit_inline_box(0, |inline_box| inline_box.height = 20.);
    assert!(Arc::ptr_eq(&shaped_text, layout.shaped_text()));

    layout.edit_inline_box(0, |inline_box| inline_box.width = 25.);
    assert!(!Arc::ptr_eq(&shaped_text, layout.shaped_text()));
    assert_eq!(advances(&shaped_text), shaped);
    assert_eq!(advances(layout.shaped_text())[1], 25.);
}
