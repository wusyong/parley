// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! machopo: tests for [`Layout::set_line_after_final_break`].

use super::utils::ColorBrush;
use super::utils::fonts::{FONT_FAMILY_LIST, create_font_context};
use crate::{
    FontContext, FontFamily, InlineBox, InlineBoxKind, Layout, LayoutContext, LineHeight,
    StyleProperty, VerticalAlign,
};

fn build_into(
    layout: &mut Layout<ColorBrush>,
    fcx: &mut FontContext,
    text: &str,
    box_at: Option<usize>,
) {
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut builder = lcx.ranged_builder(fcx, text, 1., false);
    builder.push_default(FontFamily::from(FONT_FAMILY_LIST));
    builder.push_default(StyleProperty::FontSize(20.));
    builder.push_default(LineHeight::Absolute(30.));
    if let Some(index) = box_at {
        builder.push_inline_box(InlineBox {
            id: 0,
            kind: InlineBoxKind::OutOfFlow,
            index,
            width: 10.,
            height: 10.,
            baseline: None,
            vertical_align: VerticalAlign::BASELINE,
        });
    }
    builder.build_into(layout, text);
}

/// The number of lines of `text` and their height, broken all at once and one line at a time,
/// the line after a final break made or not.
fn broken(text: &str, box_at: Option<usize>, made: bool) -> (usize, f32) {
    let mut fcx = create_font_context();
    let mut layout = Layout::new();
    build_into(&mut layout, &mut fcx, text, box_at);
    layout.set_line_after_final_break(made);
    let mut yielded = 0;
    let mut breaker = layout.break_lines();
    breaker.state_mut().set_layout_max_advance(1000.);
    breaker.state_mut().set_line_max_advance(1000.);
    while breaker.break_next().is_some() {
        yielded += 1;
    }
    breaker.finish();
    assert_eq!(yielded, layout.len(), "{text:?}");
    let mut all = layout.clone();
    all.break_all_lines(Some(1000.));
    assert_eq!(all.len(), layout.len(), "{text:?}");
    assert_eq!(all.height(), layout.height(), "{text:?}");
    (layout.len(), layout.height())
}

#[test]
fn the_line_after_a_final_break_is_not_made_when_not_asked() {
    assert_eq!(broken("a\n", None, false), (1, 30.));
    assert_eq!(broken("\n", None, false), (1, 30.));
    assert_eq!(broken("a\n\n", None, false), (2, 60.));
    // A line with text after the break is not that line.
    assert_eq!(broken("a\nb", None, false), (2, 60.));
}

#[test]
fn the_first_line_is_always_made() {
    // It has no items either, but no line comes before it.
    assert_eq!(broken("", None, false), (1, 30.));
}

#[test]
fn the_line_after_a_final_break_is_made_by_default() {
    assert_eq!(broken("a\n", None, true), (2, 60.));
    let mut fcx = create_font_context();
    let mut layout = Layout::new();
    build_into(&mut layout, &mut fcx, "a\n", None);
    layout.break_all_lines(None);
    assert_eq!(layout.len(), 2);
    // Built into again, a layout is as new.
    layout.set_line_after_final_break(false);
    build_into(&mut layout, &mut fcx, "a\n", None);
    layout.break_all_lines(None);
    assert_eq!(layout.len(), 2);
}

#[test]
fn a_line_with_a_box_after_a_final_break_is_made() {
    // The line is not empty: it stays, with no height (an invisible line box).
    assert_eq!(broken("a\n", Some(2), false), (2, 30.));
}
