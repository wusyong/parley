// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! machopo: tests for [`StyleProperty::FontFamilyIds`].

use std::{borrow::Cow, ops::Range, vec::Vec};

use fontique::FamilyId;

use super::utils::ColorBrush;
use super::utils::fonts::create_font_context;
use crate::{
    FontContext, FontFamily, FontFamilyName, Layout, LayoutContext, StyleProperty, TextStyle,
};

const TEXT: &str = "Latin \u{645}\u{631}\u{62d}\u{628}\u{627} text";

fn id(fcx: &mut FontContext, name: &str) -> FamilyId {
    fcx.collection
        .family_id(name)
        .unwrap_or_else(|| panic!("{name} font not found"))
}

fn ids<'a>(fcx: &mut FontContext, names: &[&str]) -> StyleProperty<'a, ColorBrush> {
    StyleProperty::FontFamilyIds(Cow::Owned(names.iter().map(|name| id(fcx, name)).collect()))
}

fn names<'a>(names: &[&'a str]) -> StyleProperty<'a, ColorBrush> {
    let names: Vec<_> = names
        .iter()
        .map(|name| FontFamilyName::named(name))
        .collect();
    StyleProperty::FontFamily(FontFamily::List(Cow::Owned(names)))
}

/// The font of each run of `layout` (its data's id and its index there), the text it covers
/// and its advance.
fn runs(mut layout: Layout<ColorBrush>) -> Vec<((u64, u32), Range<usize>, f32)> {
    layout.break_all_lines(None);
    layout
        .lines()
        .flat_map(|line| line.runs().collect::<Vec<_>>())
        .map(|run| {
            let font = run.font();
            (
                (font.data.id(), font.index),
                run.text_range(),
                run.advance(),
            )
        })
        .collect()
}

/// `TEXT` in `default`, the bytes in `span` in `inner` if given.
fn ranged(
    fcx: &mut FontContext,
    default: StyleProperty<'_, ColorBrush>,
    inner: Option<(StyleProperty<'_, ColorBrush>, Range<usize>)>,
) -> Vec<((u64, u32), Range<usize>, f32)> {
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut builder = lcx.ranged_builder(fcx, TEXT, 1., false);
    builder.push_default(default);
    if let Some((inner, range)) = inner {
        builder.push(inner, range);
    }
    runs(builder.build(TEXT))
}

#[test]
fn ids_pick_the_fonts_their_names_do() {
    let mut fcx = create_font_context();
    for list in [
        &["Roboto", "Noto Kufi Arabic"][..],
        &["Noto Kufi Arabic", "Roboto"],
        &["Arimo", "Noto Kufi Arabic"],
        &["Noto Kufi Arabic"],
    ] {
        let by_id = ids(&mut fcx, list);
        let by_id = ranged(&mut fcx, by_id, None);
        assert_eq!(by_id, ranged(&mut fcx, names(list), None), "{list:?}");
    }
    // Which those are depends on the list.
    let roboto = ids(&mut fcx, &["Roboto", "Noto Kufi Arabic"]);
    let arimo = ids(&mut fcx, &["Arimo", "Noto Kufi Arabic"]);
    assert_ne!(
        ranged(&mut fcx, roboto, None),
        ranged(&mut fcx, arimo, None)
    );
}

#[test]
fn ids_and_names_replace_each_other_over_a_range() {
    let mut fcx = create_font_context();
    let all = &["Roboto", "Noto Kufi Arabic"];
    let range = 0..5;
    let inner = ids(&mut fcx, &["Arimo"]);
    let ids_over_names = ranged(&mut fcx, names(all), Some((inner, range.clone())));
    let outer = ids(&mut fcx, all);
    let names_over_ids = ranged(&mut fcx, outer, Some((names(&["Arimo"]), range.clone())));
    let names_over_names = ranged(&mut fcx, names(all), Some((names(&["Arimo"]), range)));
    assert_eq!(ids_over_names, names_over_names);
    assert_eq!(names_over_ids, names_over_names);
    // The range is in another font than the text after it.
    assert_ne!(names_over_names, ranged(&mut fcx, names(all), None));
}

/// `TEXT` in a tree: `root`'s families, the first word in a span with `span`.
fn tree(
    fcx: &mut FontContext,
    root: &[&'static str],
    span: StyleProperty<'_, ColorBrush>,
) -> Vec<((u64, u32), Range<usize>, f32)> {
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let root_names: Vec<_> = root
        .iter()
        .map(|name| FontFamilyName::named(name))
        .collect();
    let root = TextStyle {
        font_family: FontFamily::List(Cow::Owned(root_names)),
        ..TextStyle::default()
    };
    let mut builder = lcx.tree_builder(fcx, 1., false, &root);
    builder.push_style_modification_span(&[span]);
    builder.push_text(&TEXT[..5]);
    builder.pop_style_span();
    builder.push_text(&TEXT[5..]);
    runs(builder.build().0)
}

#[test]
fn a_spans_ids_are_its_families() {
    let mut fcx = create_font_context();
    let all = &["Roboto", "Noto Kufi Arabic"];
    let by_id = ids(&mut fcx, &["Arimo"]);
    let by_id = tree(&mut fcx, all, by_id);
    assert_eq!(by_id, tree(&mut fcx, all, names(&["Arimo"])));
    assert_ne!(by_id, tree(&mut fcx, all, names(all)));
    // A root with no families takes them from a span around all of its text.
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let root = TextStyle {
        font_family: FontFamily::List(Cow::Borrowed(&[])),
        ..TextStyle::default()
    };
    let outer = ids(&mut fcx, all);
    let inner = ids(&mut fcx, &["Arimo"]);
    let mut builder = lcx.tree_builder(&mut fcx, 1., false, &root);
    builder.push_style_modification_span(&[outer]);
    builder.push_style_modification_span(&[inner]);
    builder.push_text(&TEXT[..5]);
    builder.pop_style_span();
    builder.push_text(&TEXT[5..]);
    builder.pop_style_span();
    assert_eq!(runs(builder.build().0), by_id);
}

#[test]
fn no_ids_are_no_families() {
    let mut fcx = create_font_context();
    let none = StyleProperty::FontFamilyIds(Cow::Borrowed(&[]));
    assert_eq!(
        ranged(&mut fcx, none, None),
        ranged(&mut fcx, names(&[]), None)
    );
}
