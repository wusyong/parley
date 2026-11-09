// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! machopo: tests for how lines break after soft hyphens ([`SoftHyphens`]).

use alloc::vec::Vec;

use super::utils::ColorBrush;
use super::utils::fonts::{FONT_FAMILY_LIST, create_font_context};
use crate::{
    ContentWidths, FontFamily, IndentOptions, InlineBox, InlineBoxKind, Layout, LayoutContext,
    LineHeight, OverflowWrap, PositionedLayoutItem, SoftHyphens, TextStyle, VerticalAlign,
};

fn style(soft_hyphens: SoftHyphens) -> TextStyle<'static, 'static, ColorBrush> {
    TextStyle {
        font_family: FontFamily::from(FONT_FAMILY_LIST),
        font_size: 20.,
        line_height: LineHeight::Absolute(30.),
        soft_hyphens,
        ..TextStyle::default()
    }
}

/// A piece of a paragraph: text, or an inline box of a kind with a width.
#[derive(Clone, Copy)]
enum Piece {
    Text(&'static str),
    Box(InlineBoxKind, f32),
}
use Piece::{Box, Text};

fn build(
    pieces: &[Piece],
    style: &TextStyle<'_, '_, ColorBrush>,
    scale: f32,
) -> Layout<ColorBrush> {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut builder = lcx.tree_builder(&mut fcx, scale, false, style);
    for (id, piece) in pieces.iter().enumerate() {
        match *piece {
            Text(text) => builder.push_text(text),
            Box(kind, width) => builder.push_inline_box(InlineBox {
                id: id as u64,
                kind,
                index: 0,
                width,
                height: 10.,
                baseline: None,
                vertical_align: VerticalAlign::BASELINE,
            }),
        }
    }
    let (layout, _) = builder.build();
    layout
}

/// Each line of `pieces`: the bytes of the text on it, its advance and its hyphen's.
fn lines_of(
    pieces: &[Piece],
    style: &TextStyle<'_, '_, ColorBrush>,
    max: Option<f32>,
) -> Vec<(core::ops::Range<usize>, f32, f32)> {
    let mut layout = build(pieces, style, 1.);
    layout.break_all_lines(max);
    layout
        .lines()
        .map(|line| {
            let metrics = line.metrics();
            (line.text_range(), metrics.advance, metrics.hyphen_advance)
        })
        .collect()
}

fn lines(
    text: &'static str,
    soft_hyphens: SoftHyphens,
    max: Option<f32>,
) -> Vec<(core::ops::Range<usize>, f32, f32)> {
    lines_of(&[Text(text)], &style(soft_hyphens), max)
}

/// The advance of `text` on one line.
fn advance(text: &'static str) -> f32 {
    let lines = lines(text, SoftHyphens::Break, None);
    assert_eq!(lines.len(), 1, "{text:?}");
    lines[0].1
}

fn widths(text: &'static str, soft_hyphens: SoftHyphens) -> ContentWidths {
    build(&[Text(text)], &style(soft_hyphens), 1.).calculate_content_widths()
}

fn assert_close(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-3, "{a} != {b}");
}

// "aaa", a soft hyphen (2 bytes), "bbb".
const WORD: &str = "aaa\u{AD}bbb";

#[test]
fn a_soft_hyphen_takes_up_no_room() {
    assert_close(advance(WORD), advance("aaabbb"));
}

#[test]
fn a_plain_break_after_a_soft_hyphen_has_no_hyphen() {
    let aaa = advance("aaa");
    let lines = lines(WORD, SoftHyphens::Break, Some(aaa + 1.));
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].0, 0..5);
    assert_close(lines[0].1, aaa);
    assert_eq!(lines[0].2, 0.);
    assert_eq!(lines[1].2, 0.);
}

#[test]
fn a_line_broken_after_a_soft_hyphen_ends_with_the_hyphen() {
    let (aaa, bbb) = (advance("aaa"), advance("bbb"));
    // The hyphen just fits.
    let lines = lines(WORD, SoftHyphens::Hyphen(7.), Some(aaa + 7.));
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].0, 0..5);
    assert_close(lines[0].1, aaa + 7.);
    assert_eq!(lines[0].2, 7.);
    // The line after has none: it does not end at a soft hyphen.
    assert_close(lines[1].1, bbb);
    assert_eq!(lines[1].2, 0.);
}

#[test]
fn a_line_that_is_not_broken_after_its_soft_hyphen_has_no_hyphen() {
    let lines = lines(WORD, SoftHyphens::Hyphen(7.), None);
    assert_eq!(lines.len(), 1);
    assert_close(lines[0].1, advance("aaabbb"));
    assert_eq!(lines[0].2, 0.);

    // Broken at a later opportunity.
    let text = "aaa\u{AD}bbb ccc";
    let lines = self::lines(text, SoftHyphens::Hyphen(7.), Some(advance("aaabbb ") + 1.));
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].0, 0..9);
    assert_close(lines[0].1, advance("aaabbb "));
    assert_eq!(lines[0].2, 0.);
}

#[test]
fn a_hyphen_that_does_not_fit_leaves_the_break_to_an_earlier_opportunity() {
    let text = "aaaa aa\u{AD}bb";
    let max = advance("aaaa aa") + 6.;
    assert!(advance("aabb") < max);
    let hyphenated = lines(text, SoftHyphens::Hyphen(7.), Some(max));
    assert_eq!(hyphenated.len(), 2);
    assert_eq!(hyphenated[0].0, 0..5);
    assert_eq!(hyphenated[0].2, 0.);
    assert_eq!(hyphenated[1].2, 0.);

    // Without a hyphen the break is after the soft hyphen.
    let plain = lines(text, SoftHyphens::Break, Some(max));
    assert_eq!(plain[0].0, 0..9);

    // A hyphen that takes up exactly what is left fits.
    let fits = lines(text, SoftHyphens::Hyphen(6.), Some(max));
    assert_eq!(fits[0].0, 0..9);
    assert_eq!(fits[0].2, 6.);
}

#[test]
fn a_hyphen_that_does_not_fit_overflows_a_line_without_another_opportunity() {
    let aaa = advance("aaa");
    let lines = lines(WORD, SoftHyphens::Hyphen(7.), Some(aaa + 6.));
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].0, 0..5);
    assert_close(lines[0].1, aaa + 7.);
    assert_eq!(lines[0].2, 7.);
}

#[test]
fn a_soft_hyphen_without_an_opportunity_does_not_break() {
    let aaa = advance("aaa");
    let unbroken = lines(WORD, SoftHyphens::NoBreak, Some(aaa + 1.));
    assert_eq!(unbroken.len(), 1);

    // The opportunities elsewhere are as they were.
    let text = "aa aaa\u{AD}bbb";
    let spaced = lines(text, SoftHyphens::NoBreak, Some(advance("aa aaa") + 1.));
    assert_eq!(spaced.len(), 2);
    assert_eq!(spaced[0].0, 0..3);

    // An emergency break can still be taken there, without a hyphen.
    let style = TextStyle {
        overflow_wrap: OverflowWrap::Anywhere,
        ..style(SoftHyphens::NoBreak)
    };
    let emergency = lines_of(&[Text(WORD)], &style, Some(aaa + 1.));
    assert_eq!(emergency[0].0, 0..5);
    assert!(emergency.iter().all(|line| line.2 == 0.));
}

#[test]
fn the_hyphen_is_scaled() {
    let mut layout = build(&[Text(WORD)], &style(SoftHyphens::Hyphen(7.)), 2.);
    layout.break_all_lines(Some(2. * advance("aaa") + 14.));
    let line = layout.lines().next().unwrap();
    assert_eq!(line.metrics().hyphen_advance, 14.);
    assert_eq!(layout.len(), 2);
    // One that fit unscaled does not when only the layout is.
    let mut layout = build(&[Text(WORD)], &style(SoftHyphens::Hyphen(7.)), 1.);
    layout.break_all_lines(Some(advance("aaa") + 14.));
    assert_eq!(layout.lines().next().unwrap().metrics().hyphen_advance, 7.);
}

#[test]
fn content_widths_count_the_hyphen() {
    let (aaaa, bb, whole) = (advance("aaaa"), advance("bb"), advance("aaaabb"));
    let text = "aaaa\u{AD}bb";
    assert!(aaaa > bb + 7.);

    let plain = widths(text, SoftHyphens::Break);
    assert_close(plain.min, aaaa);
    assert_close(plain.max, whole);

    let hyphenated = widths(text, SoftHyphens::Hyphen(7.));
    assert_close(hyphenated.min, aaaa + 7.);
    assert_close(hyphenated.max, whole);

    let unbroken = widths(text, SoftHyphens::NoBreak);
    assert_close(unbroken.min, whole);
    assert_close(unbroken.max, whole);

    // The fragment after the soft hyphen has no hyphen.
    let text = "aa\u{AD}bbbb";
    let hyphenated = widths(text, SoftHyphens::Hyphen(7.));
    assert_close(hyphenated.min, advance("bbbb"));

    // A fragment that an outdent keeps from ending has none either.
    let outdented = |soft_hyphens| {
        let mut layout = build(&[Text("aa\u{AD}bbbb")], &style(soft_hyphens), 1.);
        layout.set_text_indent(-(advance("aa") + 5.), IndentOptions::default());
        layout.calculate_content_widths()
    };
    let plain = outdented(SoftHyphens::Break).min;
    assert!(plain > 0.);
    assert_close(outdented(SoftHyphens::Hyphen(7.)).min, plain);

    // An emergency break is plain.
    let style = TextStyle {
        overflow_wrap: OverflowWrap::Anywhere,
        ..style(SoftHyphens::NoBreak)
    };
    let emergency = build(&[Text(WORD)], &style, 1.).calculate_content_widths();
    assert!(emergency.min < advance("aa"));
}

#[test]
fn a_start_edge_takes_the_opportunity_after_a_soft_hyphen_with_the_hyphen() {
    let (aaa, bbb) = (advance("aaa"), advance("bbb"));
    let pieces = [
        Text("aaa\u{AD}"),
        Box(InlineBoxKind::StartEdge, 5.),
        Text("bbb"),
        Box(InlineBoxKind::EndEdge, 5.),
    ];
    let hyphenated = style(SoftHyphens::Hyphen(7.));
    let lines = lines_of(&pieces, &hyphenated, Some(aaa + 8.));
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].0, 0..5);
    assert_close(lines[0].1, aaa + 7.);
    assert_eq!(lines[0].2, 7.);
    assert_close(lines[1].1, bbb + 10.);

    let widths = build(&pieces, &hyphenated, 1.).calculate_content_widths();
    assert_close(widths.min, (aaa + 7.).max(bbb + 10.));

    // There is none to take where the soft hyphen has none.
    let unbroken = style(SoftHyphens::NoBreak);
    assert_eq!(lines_of(&pieces, &unbroken, Some(aaa + 8.)).len(), 1);
    let widths = build(&pieces, &unbroken, 1.).calculate_content_widths();
    assert_close(widths.min, aaa + bbb + 10.);
}

#[test]
fn an_end_edge_keeps_the_soft_hyphen_before_it() {
    let (aaa, bbb) = (advance("aaa"), advance("bbb"));
    let pieces = [
        Box(InlineBoxKind::StartEdge, 5.),
        Text("aaa\u{AD}"),
        Box(InlineBoxKind::EndEdge, 5.),
        Text("bbb"),
    ];
    let hyphenated = style(SoftHyphens::Hyphen(7.));
    let lines = lines_of(&pieces, &hyphenated, Some(aaa + 18.));
    assert_eq!(lines.len(), 2);
    assert_close(lines[0].1, aaa + 17.);
    assert_eq!(lines[0].2, 7.);
    assert_close(lines[1].1, bbb);

    let widths = build(&pieces, &hyphenated, 1.).calculate_content_widths();
    assert_close(widths.min, aaa + 17.);
}

#[test]
fn the_opportunities_beside_an_inline_object_are_its_own() {
    let (aaa, bbb) = (advance("aaa"), advance("bbb"));
    let pieces = [
        Text("aaa\u{AD}"),
        Box(InlineBoxKind::InFlow, 40.),
        Text("bbb"),
    ];
    for soft_hyphens in [SoftHyphens::Hyphen(7.), SoftHyphens::NoBreak] {
        let style = style(soft_hyphens);
        let lines = lines_of(&pieces, &style, Some(aaa + 8.));
        assert_eq!(lines.len(), 3, "{soft_hyphens:?}");
        assert_close(lines[0].1, aaa);
        assert_close(lines[1].1, 40.);
        assert_close(lines[2].1, bbb);
        assert!(lines.iter().all(|line| line.2 == 0.), "{soft_hyphens:?}");

        let widths = build(&pieces, &style, 1.).calculate_content_widths();
        assert_close(widths.min, aaa.max(40.).max(bbb));
    }
}

#[test]
fn in_a_right_to_left_paragraph_the_hyphen_is_left_of_the_items() {
    // Three letters, a soft hyphen, three letters.
    let text = "\u{628}\u{628}\u{628}\u{AD}\u{62A}\u{62A}\u{62A}";
    let first = advance("\u{628}\u{628}\u{628}");
    let first_run = |soft_hyphens| {
        let mut layout = build(&[Text(text)], &style(soft_hyphens), 1.);
        layout.break_all_lines(Some(first + 8.));
        assert_eq!(layout.len(), 2);
        let line = layout.lines().next().unwrap();
        let x = line
            .items()
            .find_map(|item| match item {
                PositionedLayoutItem::GlyphRun(run) => Some(run.offset()),
                PositionedLayoutItem::InlineBox(_) => None,
            })
            .unwrap();
        (x, line.metrics().advance, line.metrics().hyphen_advance)
    };
    let (plain_x, plain_advance, _) = first_run(SoftHyphens::Break);
    let (x, advance, hyphen) = first_run(SoftHyphens::Hyphen(7.));
    assert_eq!(hyphen, 7.);
    assert_close(advance, plain_advance + 7.);
    // The items end at the right edge either way.
    assert_close(x, plain_x);
    assert!(x > 7.);
}
