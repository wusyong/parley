// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! machopo: tests for tab stops ([`TabStops`]).

use alloc::vec::Vec;

use super::utils::ColorBrush;
use super::utils::fonts::{FONT_FAMILY_LIST, create_font_context};
use crate::{
    FontFamily, Layout, LayoutContext, LineHeight, PositionedLayoutItem, TabStops, TextStyle,
    TextWrapMode,
};

fn style(tab_stops: Option<TabStops>) -> TextStyle<'static, 'static, ColorBrush> {
    TextStyle {
        font_family: FontFamily::from(FONT_FAMILY_LIST),
        font_size: 20.,
        line_height: LineHeight::Absolute(30.),
        tab_stops,
        ..TextStyle::default()
    }
}

fn stops(interval: f32, min_advance: f32) -> Option<TabStops> {
    Some(TabStops {
        interval,
        min_advance,
    })
}

fn build(text: &str, style: &TextStyle<'_, '_, ColorBrush>, scale: f32) -> Layout<ColorBrush> {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut builder = lcx.tree_builder(&mut fcx, scale, false, style);
    builder.push_text(text);
    let (layout, _) = builder.build();
    layout
}

/// The advance of each line of `text`, trailing whitespace included.
fn lines(text: &str, style: &TextStyle<'_, '_, ColorBrush>, max: Option<f32>) -> Vec<f32> {
    let mut layout = build(text, style, 1.);
    layout.break_all_lines(max);
    layout.lines().map(|line| line.metrics().advance).collect()
}

/// The advance of `text` on one line.
fn advance(text: &str, tab_stops: Option<TabStops>) -> f32 {
    let lines = lines(text, &style(tab_stops), None);
    assert_eq!(lines.len(), 1, "{text:?}");
    lines[0]
}

fn assert_close(a: f32, b: f32) {
    assert!((a - b).abs() < 1e-3, "{a} != {b}");
}

#[test]
fn a_tab_reaches_the_next_tab_stop() {
    let (a, b) = (advance("a", None), advance("b", None));
    assert!(a < 30.);
    assert_close(advance("a\tb", stops(40., 0.)), 40. + b);
    assert_close(advance("a\t\tb", stops(40., 0.)), 80. + b);
    assert_close(advance("a\tb\tb", stops(40., 0.)), 80. + b);
    // At a tab stop, the tab reaches the one after it.
    assert_close(advance("\t\tb", stops(40., 0.)), 80. + b);
}

#[test]
fn without_tab_stops_a_tab_keeps_the_advance_it_was_shaped_with() {
    // It is shaped as a space.
    assert_close(advance("a\tb", None), advance("a b", None));
}

#[test]
fn a_tab_nearer_its_stop_than_the_least_advance_reaches_the_one_after() {
    let (a, b) = (advance("a", None), advance("b", None));
    let interval = a + 2.;
    assert_close(advance("a\tb", stops(interval, 1.5)), interval + b);
    assert_close(advance("a\tb", stops(interval, 2.5)), 2. * interval + b);
    // A stop as far as the least advance is reached.
    assert_close(advance("\tb", stops(40., 40.)), 40. + b);
}

#[test]
fn a_tab_without_an_interval_has_only_its_letter_spacing() {
    let spaced = |text, tab_stops| {
        let style = TextStyle {
            letter_spacing: 5.,
            ..style(tab_stops)
        };
        lines(text, &style, None)[0]
    };
    assert_close(
        advance("a\tb", stops(0., 3.)),
        advance("a", None) + advance("b", None),
    );
    assert_close(
        spaced("a\tb", stops(0., 3.)),
        spaced("a", None) + spaced("b", None) + 5.,
    );
}

#[test]
fn a_tab_is_placed_from_the_start_of_its_line() {
    let (b, cc) = (advance("b", None), advance("cc", None));
    assert!(cc < 40.);
    // After a forced break, and after a soft wrap.
    let forced = lines("a\tb\ncc\tb", &style(stops(40., 0.)), None);
    assert_close(forced[0], 40. + b);
    assert_close(forced[1], 40. + b);

    let wrapped = lines(
        "aaaaaa cc\tb",
        &style(stops(40., 0.)),
        Some(advance("aaaaaa ", None)),
    );
    assert_eq!(wrapped.len(), 2);
    assert_close(wrapped[1], 40. + b);
}

#[test]
fn a_tab_that_moves_is_measured_again() {
    // The same layout broken at another width: the tab is first after "aaaaaa cc", then after
    // "cc" at the start of a line.
    let b = advance("b", None);
    let tab_stops = stops(40., 0.);
    let mut layout = build("aaaaaa cc\tb", &style(tab_stops), 1.);
    layout.break_all_lines(None);
    let unbroken = layout.full_width();
    layout.break_all_lines(Some(advance("aaaaaa ", None)));
    assert_close(layout.lines().nth(1).unwrap().metrics().advance, 40. + b);
    layout.break_all_lines(None);
    assert_close(layout.full_width(), unbroken);
}

#[test]
fn tab_stops_are_scaled() {
    let b = advance("b", None);
    let mut layout = build("a\tb", &style(stops(40., 0.)), 2.);
    layout.break_all_lines(None);
    assert_close(layout.full_width(), 80. + 2. * b);

    // The least advance too.
    let a = advance("a", None);
    let mut layout = build("a\tb", &style(stops(a + 2., 1.5)), 2.);
    layout.break_all_lines(None);
    assert_close(layout.full_width(), 2. * (a + 2.) + 2. * b);
    let mut layout = build("a\tb", &style(stops(a + 2., 2.5)), 2.);
    layout.break_all_lines(None);
    assert_close(layout.full_width(), 4. * (a + 2.) + 2. * b);
}

#[test]
fn content_widths_place_tabs() {
    let b = advance("b", None);
    let tab_stops = stops(40., 0.);
    let widths = |text, wrap| {
        let style = TextStyle {
            text_wrap_mode: wrap,
            ..style(tab_stops)
        };
        build(text, &style, 1.).calculate_content_widths()
    };
    let nowrap = widths("a\tb\tb", TextWrapMode::NoWrap);
    assert_close(nowrap.max, 80. + b);
    assert_close(nowrap.min, 80. + b);

    // Each of the two widths places the tab on its own line: for the least one that is the line
    // "cc\tb" (which does not wrap), for the greatest the whole text.
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let root = style(tab_stops);
    let mut builder = lcx.tree_builder(&mut fcx, 1., false, &root);
    builder.push_text("aa ");
    builder.push_style_span(TextStyle {
        text_wrap_mode: TextWrapMode::NoWrap,
        ..root.clone()
    });
    builder.push_text("cc\tb");
    builder.pop_style_span();
    let (layout, _) = builder.build();
    let wrap = layout.calculate_content_widths();
    assert_close(wrap.max, advance("aa cc\tb", tab_stops));
    assert!(advance("aa", None) < 40.);
    assert_close(wrap.min, 40. + b);
}

#[test]
fn the_glyphs_after_a_tab_are_at_its_stop() {
    let mut layout = build("a\tb", &style(stops(40., 0.)), 1.);
    layout.break_all_lines(None);
    let line = layout.lines().next().unwrap();
    let glyphs: Vec<_> = line
        .items()
        .filter_map(|item| match item {
            PositionedLayoutItem::GlyphRun(run) => Some(run),
            PositionedLayoutItem::InlineBox(_) => None,
        })
        .flat_map(|run| run.positioned_glyphs())
        .collect();
    assert_eq!(glyphs.len(), 3);
    assert_close(glyphs[2].x, 40.);
}
