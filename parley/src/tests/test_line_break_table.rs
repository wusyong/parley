// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! machopo: tests for the line break opportunities looked up in a table (which texts are, and
//! how many of their words are given to ICU4X: the table's own tests compare what it gives with
//! what ICU4X does), and for leaving word boundaries out.

use alloc::vec::Vec;
use core::ops::Range;

use super::utils::ColorBrush;
use super::utils::fonts::{FONT_FAMILY_LIST, create_font_context};
use crate::{
    FontContext, FontFamily, InlineBox, InlineBoxKind, Layout, LayoutContext, StyleProperty,
    VerticalAlign, WordBreak,
};

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

    /// `text` with `break_all` over a part of it and inline objects at `objects`, each line as
    /// short as it can be.
    fn build(
        &mut self,
        text: &str,
        break_all: Option<Range<usize>>,
        objects: &[usize],
    ) -> Layout<ColorBrush> {
        let mut builder = self.lcx.ranged_builder(&mut self.fcx, text, 1., false);
        builder.push_default(StyleProperty::FontFamily(FontFamily::from(
            FONT_FAMILY_LIST,
        )));
        if let Some(range) = break_all {
            builder.push(StyleProperty::WordBreak(WordBreak::BreakAll), range);
        }
        for (id, &index) in objects.iter().enumerate() {
            builder.push_inline_box(InlineBox {
                id: id as u64,
                kind: InlineBoxKind::InFlow,
                index,
                width: 10.,
                height: 10.,
                baseline: None,
                vertical_align: VerticalAlign::BASELINE,
            });
        }
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(0.));
        layout
    }

    /// How many texts building `text` looked up in the table, and how many words it gave to
    /// ICU4X.
    fn looked_up(&mut self, text: &str) -> (usize, usize) {
        let (texts, words) = self.lcx.line_breaks_looked_up();
        self.build(text, None, &[]);
        let (now_texts, now_words) = self.lcx.line_breaks_looked_up();
        (now_texts - texts, now_words - words)
    }
}

/// The clusters marked as word boundaries in `layout`, and its lines' ranges.
fn boundaries_and_lines(layout: &Layout<ColorBrush>) -> (Vec<usize>, Vec<Range<usize>>) {
    let boundaries = layout
        .lines()
        .flat_map(|line| line.runs().collect::<Vec<_>>())
        .flat_map(|run| run.clusters().collect::<Vec<_>>())
        .filter(|cluster| cluster.is_word_boundary())
        .map(|cluster| cluster.text_range().start)
        .collect();
    let lines = layout.lines().map(|line| line.text_range()).collect();
    (boundaries, lines)
}

#[test]
fn text_in_the_table_is_looked_up() {
    let mut cx = Contexts::new();
    assert_eq!(
        cx.looked_up("Le chat dort sur le canapé, près de la fenêtre (à 5 h)."),
        (1, 0)
    );
    assert_eq!(
        cx.looked_up("L’enfant — son cœur… à Łódź, 5 ‰, un čaj, note†1."),
        (1, 0)
    );
    // The euro sign is outside the table; the word after it is looked up, but not when the sign
    // ends the word before.
    assert_eq!(cx.looked_up("Il coûte 5 €. Bien."), (1, 1));
    assert_eq!(cx.looked_up("Il coûte 5 €… Bien."), (1, 1));
    assert_eq!(cx.looked_up("€ 5"), (1, 2));
    // An opening quotation mark keeps the word after it on its line depending on what is before
    // the mark; a closing one, on what is after.
    assert_eq!(cx.looked_up("Il dit « oui »."), (1, 2));
    assert_eq!(cx.looked_up("Il dit «\u{a0}oui\u{a0}»."), (1, 0));
}

#[test]
fn text_with_other_rules_is_given_to_icu4x_whole() {
    let mut cx = Contexts::new();
    let text = "Le chat dort.";
    cx.build(text, Some(0..text.len()), &[]);
    cx.build(text, Some(3..7), &[]);
    assert_eq!(cx.lcx.line_breaks_looked_up(), (0, 0));
}

#[test]
fn an_inline_object_is_outside_the_table() {
    let mut cx = Contexts::new();
    let layout = cx.build("Le chat\u{fffc}dort ici.", None, &[7]);
    assert_eq!(cx.lcx.line_breaks_looked_up(), (1, 1));
    let lines: Vec<Range<usize>> = layout.lines().map(|line| line.text_range()).collect();
    assert_eq!(lines, [0..3, 3..7, 7..10, 10..15, 15..19]);
}

#[test]
fn word_boundaries_are_left_out_when_turned_off() {
    let mut cx = Contexts::new();
    // Thai has no dictionary here: every grapheme starts a word.
    let text = "Le chat (gris) dort. L’enfant aussi. ภาษาไทย";
    let (boundaries, lines) = boundaries_and_lines(&cx.build(text, None, &[]));
    assert!(!boundaries.is_empty());
    cx.lcx.set_word_boundaries(false);
    let (no_boundaries, same_lines) = boundaries_and_lines(&cx.build(text, None, &[]));
    assert_eq!(no_boundaries, Vec::<usize>::new());
    // Line break opportunities are where they were.
    assert_eq!(same_lines, lines);
    assert!(lines.len() > 1);
}
