// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! machopo: tests for [`TreeBuilder::push_default`](crate::TreeBuilder::push_default).

use std::{borrow::Cow, vec::Vec};

use super::utils::ColorBrush;
use super::utils::fonts::create_font_context;
use crate::{
    FontContext, FontFamily, FontFamilyName, Layout, LayoutContext, StyleProperty, TextStyle,
    WhiteSpaceCollapse,
};

fn root(families: &[&'static str], size: f32) -> TextStyle<'static, 'static, ColorBrush> {
    let names: Vec<_> = families
        .iter()
        .map(|name| FontFamilyName::named(name))
        .collect();
    TextStyle {
        font_family: FontFamily::List(Cow::Owned(names)),
        font_size: size,
        ..TextStyle::default()
    }
}

fn ids<'a>(fcx: &mut FontContext, names: &[&str]) -> StyleProperty<'a, ColorBrush> {
    StyleProperty::FontFamilyIds(Cow::Owned(
        names
            .iter()
            .map(|name| {
                fcx.collection
                    .family_id(name)
                    .unwrap_or_else(|| panic!("{name} font not found"))
            })
            .collect(),
    ))
}

/// Each line's height, baseline and advance, and each run's font and advance.
type Lines = Vec<(f32, f32, f32, Vec<((u64, u32), f32)>)>;

fn lines(mut layout: Layout<ColorBrush>) -> Lines {
    layout.break_all_lines(None);
    layout
        .lines()
        .map(|line| {
            let metrics = line.metrics();
            let runs = line
                .runs()
                .map(|run| {
                    let font = run.font();
                    ((font.data.id(), font.index), run.advance())
                })
                .collect();
            (metrics.line_height, metrics.baseline, metrics.advance, runs)
        })
        .collect()
}

/// `text` in a tree with the root style `root` and the `defaults` set on it.
fn tree(
    fcx: &mut FontContext,
    root: &TextStyle<'_, '_, ColorBrush>,
    defaults: &[StyleProperty<'_, ColorBrush>],
    text: &str,
) -> Lines {
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut builder = lcx.tree_builder(fcx, 1., false, root);
    for property in defaults {
        builder.push_default(property.clone());
    }
    builder.push_text(text);
    lines(builder.build().0)
}

/// Text, no text, and a line with nothing on it after the text: the root style is the
/// style of each.
const TEXTS: [&str; 3] = ["Some text", "", "Some text\n"];

#[test]
fn a_default_is_the_root_styles() {
    let mut fcx = create_font_context();
    let plain = root(&["Roboto"], 16.);
    for text in TEXTS {
        let set = tree(&mut fcx, &plain, &[StyleProperty::FontSize(40.)], text);
        assert_eq!(
            set,
            tree(&mut fcx, &root(&["Roboto"], 40.), &[], text),
            "{text:?}"
        );
        assert_ne!(set, tree(&mut fcx, &plain, &[], text), "{text:?}");
    }
}

#[test]
fn default_family_ids_are_the_root_styles_families() {
    let mut fcx = create_font_context();
    let none = root(&[], 16.);
    for text in TEXTS {
        let kufi = ids(&mut fcx, &["Noto Kufi Arabic"]);
        let kufi = tree(&mut fcx, &none, &[kufi], text);
        assert_eq!(
            kufi,
            tree(&mut fcx, &root(&["Noto Kufi Arabic"], 16.), &[], text),
            "{text:?}"
        );
        // The line with nothing on it is as high as the font's, not as another's.
        let roboto = ids(&mut fcx, &["Roboto"]);
        let roboto = tree(&mut fcx, &none, &[roboto], text);
        assert_eq!(
            roboto,
            tree(&mut fcx, &root(&["Roboto"], 16.), &[], text),
            "{text:?}"
        );
        assert_ne!(kufi, roboto, "{text:?}");
    }
}

/// A span takes the root style as it is when the span is pushed, the default included.
#[test]
fn spans_inherit_the_default() {
    let mut fcx = create_font_context();
    let build = |fcx: &mut FontContext, root: &TextStyle<'_, '_, ColorBrush>, size: Option<f32>| {
        let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
        let mut builder = lcx.tree_builder(fcx, 1., false, root);
        if let Some(size) = size {
            builder.push_default(StyleProperty::FontSize(size));
        }
        builder.push_style_modification_span(&[StyleProperty::LetterSpacing(1.)]);
        builder.push_text("Some");
        builder.pop_style_span();
        builder.push_text(" text");
        lines(builder.build().0)
    };
    let set = build(&mut fcx, &root(&["Roboto"], 16.), Some(40.));
    assert_eq!(set, build(&mut fcx, &root(&["Roboto"], 40.), None));
    assert_ne!(set, build(&mut fcx, &root(&["Roboto"], 16.), None));
}

#[test]
#[should_panic(expected = "the root style is set before anything is pushed")]
fn a_default_after_text_panics() {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut builder = lcx.tree_builder(&mut fcx, 1., false, &root(&["Roboto"], 16.));
    builder.push_text("Some text");
    builder.push_default(StyleProperty::FontSize(40.));
}

#[test]
#[should_panic(expected = "the root style is set before anything is pushed")]
fn a_default_inside_a_span_panics() {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let mut builder = lcx.tree_builder(&mut fcx, 1., false, &root(&["Roboto"], 16.));
    builder.push_style_modification_span(&[StyleProperty::LetterSpacing(1.)]);
    builder.push_default(StyleProperty::FontSize(40.));
}

/// White space waiting to collapse into what follows is text pushed too.
#[test]
#[should_panic(expected = "the root style is set before anything is pushed")]
fn a_default_after_white_space_panics() {
    let mut fcx = create_font_context();
    let mut lcx: LayoutContext<ColorBrush> = LayoutContext::new();
    let root = TextStyle {
        white_space_collapse: WhiteSpaceCollapse::Collapse,
        ..root(&["Roboto"], 16.)
    };
    let mut builder = lcx.tree_builder(&mut fcx, 1., false, &root);
    builder.push_text(" ");
    assert_eq!(builder.text(), "");
    builder.push_default(StyleProperty::FontSize(40.));
}
