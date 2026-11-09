// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! machopo: tests for optical sizing ([`TextStyle::optical_sizing`]).

use alloc::borrow::Cow;
use alloc::vec::Vec;

use super::utils::ColorBrush;
use super::utils::fonts::create_font_context;
use crate::setting::Tag;
use crate::{
    FontFamily, FontVariation, FontVariations, LayoutContext, PositionedLayoutItem, TextStyle,
};

/// A span of "aa": whether it is sized optically, and its `opsz` variation, if any.
type Span = (bool, Option<f32>);

/// The normalized coords and the advance of each glyph run of `spans`, in `family` at 40px.
fn runs(family: &'static str, spans: &[Span]) -> Vec<(Vec<i16>, f32)> {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let root = TextStyle {
        font_family: FontFamily::named(family),
        font_size: 40.,
        ..TextStyle::default()
    };
    let mut builder = lcx.tree_builder(&mut fcx, 1., false, &root);
    for &(optical_sizing, opsz) in spans {
        let variations = opsz.map(|value| {
            [FontVariation {
                tag: Tag::new(b"opsz"),
                value,
            }]
        });
        builder.push_style_span(TextStyle {
            optical_sizing,
            font_variations: match &variations {
                Some(list) => FontVariations::List(Cow::Borrowed(list)),
                None => FontVariations::empty(),
            },
            ..root.clone()
        });
        builder.push_text("aa");
        builder.pop_style_span();
    }
    let (mut layout, _) = builder.build();
    layout.break_all_lines(None);
    layout
        .lines()
        .flat_map(|line| line.items())
        .filter_map(|item| match item {
            PositionedLayoutItem::GlyphRun(glyph_run) => Some(glyph_run),
            PositionedLayoutItem::InlineBox(_) => None,
        })
        .map(|glyph_run| {
            let coords = glyph_run.run().normalized_coords();
            (
                coords.iter().map(|coord| coord.to_bits()).collect(),
                glyph_run.advance(),
            )
        })
        .collect()
}

const FLEX: &str = "Roboto Flex";

#[test]
fn optical_sizing_sets_opsz_to_the_font_size() {
    let off = runs(FLEX, &[(false, None)]);
    let on = runs(FLEX, &[(true, None)]);
    assert_eq!(off.len(), 1);
    assert!(off[0].0.iter().all(|&coord| coord == 0), "{off:?}");
    assert_ne!(on, off);
    assert_eq!(on, runs(FLEX, &[(false, Some(40.))]));
    assert_ne!(on, runs(FLEX, &[(false, Some(30.))]));
}

#[test]
fn the_variations_own_opsz_wins() {
    let set = runs(FLEX, &[(false, Some(8.))]);
    assert_eq!(runs(FLEX, &[(true, Some(8.))]), set);
    assert_ne!(runs(FLEX, &[(true, None)]), set);
}

#[test]
fn spans_sized_differently_are_shaped_apart() {
    let both = runs(FLEX, &[(true, None), (false, None)]);
    let on = runs(FLEX, &[(true, None)]);
    let off = runs(FLEX, &[(false, None)]);
    assert_eq!(both, [on[0].clone(), off[0].clone()]);
    let both = runs(FLEX, &[(false, None), (true, None)]);
    assert_eq!(both, [off[0].clone(), on[0].clone()]);
}

#[test]
fn a_font_without_the_axis_is_as_it_was() {
    assert_eq!(
        runs("Roboto", &[(true, None)]),
        runs("Roboto", &[(false, None)])
    );
}
