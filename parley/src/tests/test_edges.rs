// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! machopo: tests for the edges of non-atomic inline boxes ([`InlineBoxKind::StartEdge`] and
//! [`InlineBoxKind::EndEdge`]).

use alloc::vec::Vec;

use super::utils::ColorBrush;
use super::utils::fonts::{FONT_FAMILY_LIST, create_font_context};
use crate::{
    Cluster, FontFamily, InlineBox, InlineBoxKind, Layout, LayoutContext, LineHeight,
    PositionedLayoutItem, TextStyle, TextWrapMode, VerticalAlign,
};

fn root_style() -> TextStyle<'static, 'static, ColorBrush> {
    TextStyle {
        font_family: FontFamily::from(FONT_FAMILY_LIST),
        font_size: 20.,
        line_height: LineHeight::Absolute(30.),
        ..TextStyle::default()
    }
}

/// A piece of a paragraph: text, or an edge with an id and a width.
#[derive(Clone, Copy)]
enum Piece {
    Text(&'static str),
    /// Text that does not wrap.
    NoWrap(&'static str),
    Start(u64, f32),
    End(u64, f32),
    OutOfFlow(u64),
}
use Piece::{End, NoWrap, OutOfFlow, Start, Text};

/// The layout of `pieces`, not broken into lines yet. The edges are taller than the text, as an
/// edge with vertical padding would be.
fn build(pieces: &[Piece]) -> Layout<ColorBrush> {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let root = root_style();
    let mut builder = lcx.tree_builder(&mut fcx, 1., false, &root);
    for piece in pieces {
        let (id, kind, width) = match *piece {
            Text(text) => {
                builder.push_text(text);
                continue;
            }
            NoWrap(text) => {
                builder.push_style_span(TextStyle {
                    text_wrap_mode: TextWrapMode::NoWrap,
                    ..root.clone()
                });
                builder.push_text(text);
                builder.pop_style_span();
                continue;
            }
            Start(id, width) => (id, InlineBoxKind::StartEdge, width),
            End(id, width) => (id, InlineBoxKind::EndEdge, width),
            OutOfFlow(id) => (id, InlineBoxKind::OutOfFlow, 40.),
        };
        builder.push_inline_box(InlineBox {
            id,
            kind,
            index: 0,
            width,
            height: 100.,
            baseline: None,
            vertical_align: VerticalAlign::BASELINE,
        });
    }
    let (layout, _) = builder.build();
    layout
}

fn broken(pieces: &[Piece], max_advance: Option<f32>) -> Layout<ColorBrush> {
    let mut layout = build(pieces);
    layout.break_all_lines(max_advance);
    layout
}

/// The advance of `text` on a line of its own, trailing whitespace included.
fn advance(text: &'static str) -> f32 {
    broken(&[Text(text)], None).full_width()
}

/// A line's items: `Ok` the x and advance of a glyph run, `Err` the id and x of a box.
fn items(layout: &Layout<ColorBrush>, line: usize) -> Vec<Result<(f32, f32), (u64, f32)>> {
    layout
        .lines()
        .nth(line)
        .unwrap()
        .items()
        .map(|item| match item {
            PositionedLayoutItem::GlyphRun(run) => Ok((run.offset(), run.advance())),
            PositionedLayoutItem::InlineBox(inline_box) => Err((inline_box.id, inline_box.x)),
        })
        .collect()
}

fn text_ranges(layout: &Layout<ColorBrush>) -> Vec<core::ops::Range<usize>> {
    layout.lines().map(|line| line.text_range()).collect()
}

#[test]
fn an_edge_takes_up_its_width_and_no_height() {
    let (ab, cd, ef) = (advance("ab"), advance("cd"), advance("ef"));
    let layout = broken(
        &[
            Text("ab"),
            Start(1, 10.),
            Text("cd"),
            End(2, 5.),
            Text("ef"),
        ],
        None,
    );
    assert_eq!(layout.len(), 1);
    assert_eq!(
        items(&layout, 0),
        [
            Ok((0., ab)),
            Err((1, ab)),
            Ok((ab + 10., cd)),
            Err((2, ab + 10. + cd)),
            Ok((ab + 10. + cd + 5., ef)),
        ]
    );
    let metrics = *layout.lines().next().unwrap().metrics();
    assert_eq!(metrics.advance, ab + cd + ef + 15.);
    assert_eq!(layout.full_width(), ab + cd + ef + 15.);
    // The edges are 100 tall, the line is as tall as its text.
    assert_eq!(metrics.line_height, 30.);
}

#[test]
fn an_edge_is_not_a_character() {
    // The text on the two sides of an edge is analyzed as if the edge weren't there: no
    // opportunity inside "abcd", and the text is the pieces' text.
    let pieces = [Text("ab"), Start(1, 10.), Text("cd"), End(2, 5.)];
    let layout = broken(&pieces, Some(1.));
    assert_eq!(layout.len(), 1);
    let widths = build(&pieces).calculate_content_widths();
    assert_eq!(widths.min, advance("ab") + advance("cd") + 15.);
    assert_eq!(widths.max, widths.min);
}

#[test]
fn a_start_edge_stays_with_the_content_after_it() {
    let pieces = [Text("aaa "), Start(1, 10.), Text("bbb"), End(2, 0.)];
    // The edge fits after "aaa ", "bbb" doesn't: the break goes before the edge.
    let layout = broken(&pieces, Some(advance("aaa ") + 11.));
    assert_eq!(text_ranges(&layout), [0..4, 4..7]);
    assert_eq!(items(&layout, 0), [Ok((0., advance("aaa ")))]);
    assert_eq!(
        items(&layout, 1),
        [
            Err((1, 0.)),
            Ok((10., advance("bbb"))),
            Err((2, 10. + advance("bbb")))
        ]
    );
    // And the edge's width is on the second line.
    assert_eq!(
        layout.lines().nth(1).unwrap().metrics().advance,
        10. + advance("bbb")
    );
}

#[test]
fn consecutive_start_edges_stay_together() {
    // Nested spans, with an out-of-flow box between their edges.
    let pieces = [
        Text("aaa "),
        Start(1, 10.),
        OutOfFlow(9),
        Start(2, 10.),
        Text("bbb"),
    ];
    let layout = broken(&pieces, Some(advance("aaa ") + 21.));
    assert_eq!(text_ranges(&layout), [0..4, 4..7]);
    assert_eq!(
        items(&layout, 1),
        [
            Err((1, 0.)),
            Err((9, 10.)),
            Err((2, 10.)),
            Ok((20., advance("bbb")))
        ]
    );
    // One opportunity, not one per edge: both edges are in the second fragment.
    let widths = build(&pieces).calculate_content_widths();
    assert_eq!(widths.min, advance("bbb") + 20.);
}

#[test]
fn an_end_edge_stays_with_the_content_before_it() {
    let pieces = [
        Text("aaa "),
        Start(1, 0.),
        Text("bbb"),
        End(2, 50.),
        Text(" ccc"),
    ];
    // "bbb" fits on the first line, its end edge doesn't: "bbb" goes with it.
    let layout = broken(&pieces, Some(advance("aaa ") + advance("bbb") + 10.));
    assert_eq!(text_ranges(&layout)[0], 0..4);
    assert_eq!(
        items(&layout, 1)[..3],
        [
            Err((1, 0.)),
            Ok((0., advance("bbb"))),
            Err((2, advance("bbb")))
        ]
    );
}

#[test]
fn an_edge_that_does_not_fit_overflows_without_an_opportunity() {
    // No opportunity on the line: the edge overflows, as text would.
    let layout = broken(&[Text("aaa"), End(1, 50.)], Some(advance("aaa") + 10.));
    assert_eq!(layout.len(), 1);
    assert_eq!(
        layout.lines().next().unwrap().metrics().advance,
        advance("aaa") + 50.
    );
    // First on a line, it stays there however wide.
    let layout = broken(&[Start(1, 50.), Text("aaa bbb")], Some(10.));
    assert_eq!(text_ranges(&layout), [0..4, 4..7]);
    assert_eq!(items(&layout, 0)[0], Err((1, 0.)));
}

#[test]
fn an_edge_after_text_that_does_not_wrap_takes_an_earlier_opportunity() {
    // As "bbb" itself would if it didn't fit: the opportunity before it is the wrapping text's.
    let pieces = [Text("aaa "), NoWrap("bbb"), End(1, 50.)];
    let layout = broken(&pieces, Some(advance("aaa bbb") + 10.));
    assert_eq!(text_ranges(&layout), [0..4, 4..7]);
    // And a start edge before text that does not wrap takes none for it.
    let pieces = [NoWrap("aaa "), Start(1, 10.), NoWrap("bbb")];
    let layout = broken(&pieces, Some(advance("aaa ")));
    assert_eq!(layout.len(), 1);
}

#[test]
fn content_widths_include_edges() {
    let pieces = [
        Text("aaa "),
        Start(1, 10.),
        Text("bbb"),
        End(2, 7.),
        Text(" c"),
    ];
    let widths = build(&pieces).calculate_content_widths();
    // The fragments are "aaa ", then both edges with "bbb" (the space after hangs), then "c".
    assert_eq!(widths.min, advance("bbb") + 17.);
    assert_eq!(
        widths.max,
        advance("aaa ") + advance("bbb") + advance(" c") + 17.
    );
    // The same as the lines the breaker makes.
    let mut layout = build(&pieces);
    layout.break_all_lines(Some(widths.min));
    assert_eq!(text_ranges(&layout), [0..4, 4..8, 8..9]);
    layout.break_all_lines(Some(widths.max));
    assert_eq!(layout.len(), 1);
}

#[test]
fn whitespace_before_an_edge_does_not_hang() {
    let space = advance("a ") - advance("a");
    assert!(space > 0.);
    let hanging = |layout: &Layout<ColorBrush>| {
        assert_eq!(text_ranges(layout), [0..4, 4..7]);
        layout.lines().next().unwrap().metrics().hanging_advance
    };
    // At the end of a wrapped line, the space hangs.
    let layout = broken(&[Text("aaa bbb")], Some(advance("aaa ")));
    assert_eq!(hanging(&layout), space);
    // With an edge after it, it is not at the end of the line.
    let pieces = [Text("aaa "), End(1, 30.), Text("bbb")];
    let layout = broken(&pieces, Some(advance("aaa ") + 30.));
    assert_eq!(hanging(&layout), 0.);
    let widths = build(&pieces).calculate_content_widths();
    assert_eq!(widths.min, advance("aaa ") + 30.);
}

#[test]
fn a_line_of_edges_alone_has_no_height() {
    let pieces = [Start(1, 10.), End(2, 5.)];
    let layout = broken(&pieces, None);
    assert_eq!(layout.len(), 1);
    let metrics = *layout.lines().next().unwrap().metrics();
    assert_eq!(metrics.line_height, 0.);
    assert_eq!(metrics.advance, 15.);
    assert_eq!(items(&layout, 0), [Err((1, 0.)), Err((2, 10.))]);
    let widths = build(&pieces).calculate_content_widths();
    assert_eq!((widths.min, widths.max), (15., 15.));
}

#[test]
fn hit_testing_skips_an_edge_s_width() {
    let ab = advance("ab");
    let layout = broken(&[Text("ab"), Start(1, 10.), Text("cd")], None);
    // "c" starts after the edge.
    let c = Cluster::from_byte_index(&layout, 2).unwrap();
    assert_eq!(c.visual_offset(), Some(ab + 10.));
    let (hit, _) = Cluster::from_point(&layout, ab + 11., 5.).unwrap();
    assert_eq!(hit.text_range(), 2..3);
    let (hit, _) = Cluster::from_point(&layout, ab - 1., 5.).unwrap();
    assert_eq!(hit.text_range(), 1..2);
}

#[test]
fn breaking_by_length_keeps_edges_widths() {
    let mut layout = build(&[Text("ab"), Start(1, 10.), Text("cd"), End(2, 5.)]);
    let mut breaker = layout.break_lines();
    while breaker.break_next_with_length(100).is_some() {}
    breaker.finish();
    assert_eq!(
        layout.lines().next().unwrap().metrics().advance,
        advance("ab") + advance("cd") + 15.
    );
}
