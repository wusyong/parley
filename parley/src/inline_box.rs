// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::VerticalAlign;

/// A box to be laid out inline with text
#[derive(PartialEq, Debug, Clone)]
pub struct InlineBox {
    /// User-specified identifier for the box, which can be used by the user to determine which box in
    /// parley's output corresponds to which box in its input.
    pub id: u64,
    /// Whether the box is in-flow (takes up space in the layout) or out-of-flow (e.g. absolutely positioned or floated)
    pub kind: InlineBoxKind,
    /// The byte offset into the underlying text string at which the box is placed.
    ///
    /// An [in-flow](InlineBoxKind::InFlow) box must be represented by a U+FFFC OBJECT REPLACEMENT
    /// CHARACTER in the text, and this must be the offset of that character. Each such character
    /// holds at most one box.
    ///
    /// Any U+FFFC character in the text without a corresponding in-flow box, is treated as ordinary
    /// text (some fonts may render .notdef, others may have a glyph for it).
    ///
    /// Out-of-flow boxes and [edges](InlineBoxKind::StartEdge) can be placed at any character
    /// boundary.
    ///
    /// When using the [`TreeBuilder`](crate::TreeBuilder), this offset is set by the builder.
    pub index: usize,
    /// The width of the box in pixels
    pub width: f32,
    /// The height of the box in pixels
    pub height: f32,
    /// The baseline of the box in pixels, relative to the top of the box
    ///
    /// If `None`, the baseline is the box's bottom edge.
    pub baseline: Option<f32>,
    /// Vertical alignment of the box within its line, relative to the enclosing style span.
    pub vertical_align: VerticalAlign,
}

/// An [`InlineBox`] together with the style index of the span that contains it.
#[derive(PartialEq, Debug, Clone)]
pub(crate) struct LayoutInlineBox {
    pub(crate) inline_box: InlineBox,
    /// Style index of the span containing the box, against which the box's `vertical_align`
    /// is resolved. The box itself has no style of its own.
    pub(crate) parent_style_index: u16,
    /// Offset of the box's baseline above the baseline of the independent aligned subtree it
    /// belongs to (that of the parent style), resolved when the layout is built and when the box
    /// is edited (see [`crate::Layout::edit_inline_box`]). Always zero for out-of-flow boxes, and
    /// unused for `vertical-align: top | bottom` boxes.
    pub(crate) baseline_offset: f32,
    /// The shaped cluster of the box's inline object, if the box is in-flow. See
    /// [`parley_engine::AnalysisOptions::inline_objects`].
    ///
    /// This is `None` for out-of-flow boxes.
    pub(crate) inline_object_cluster: Option<u32>,
}

impl LayoutInlineBox {
    pub(crate) fn new(inline_box: InlineBox, parent_style_index: u16) -> Self {
        Self {
            inline_box,
            parent_style_index,
            baseline_offset: 0.,
            inline_object_cluster: None,
        }
    }

    /// Whether the box is an inline object, i.e., an in-flow box, which is represented by a
    /// U+FFFC OBJECT REPLACEMENT CHARACTER at its index in the text.
    pub(crate) fn is_inline_object(&self) -> bool {
        self.inline_box.kind == InlineBoxKind::InFlow
    }
}

/// The index into `inline_boxes` of the inline object whose U+FFFC starts at `text_byte`.
///
/// `inline_boxes` must be sorted by index, and there must be such an inline object.
pub(crate) fn inline_object_box_index(inline_boxes: &[LayoutInlineBox], text_byte: usize) -> usize {
    let start = inline_boxes.partition_point(|b| b.inline_box.index < text_byte);
    // There may be some out-of-flow boxes before the in-flow box, so walk over them.
    let index = start
        + inline_boxes[start..]
            .iter()
            .position(LayoutInlineBox::is_inline_object)
            .expect("an inline object must have a box");
    debug_assert_eq!(
        inline_boxes[index].inline_box.index, text_byte,
        "an inline object must have an `InlineBox`"
    );
    index
}

/// Whether a box is in-flow (takes up space in the layout) or out-of-flow (e.g. absolutely positioned)
/// or custom-out-of-flow (line-breaking should yield control flow)
#[derive(PartialEq, Debug, Clone, Copy)]
pub enum InlineBoxKind {
    /// `InFlow` boxes take up space in the layout and flow in line with text
    ///
    /// They correspond to [atomic inlines] in CSS. Each such box corresponds to a U+FFFC OBJECT
    /// REPLACEMENT CHARACTER in the text (see [`InlineBox::index`]), and takes part in line
    /// breaking and selection.
    ///
    /// [atomic inline]: https://www.w3.org/TR/css-display-3/#atomic-inline
    InFlow,
    /// `OutOfFlow` boxes are assigned a position as if they were a zero-sized inline box, but
    /// do not take up space in the layout.
    ///
    /// They correspond to `position: absolute` boxes in CSS.
    OutOfFlow,
    /// `CustomOutOfFlow` boxes also do not take up space in the layout, but they are not assigned a position
    /// by Parley. When they are encountered, control flow is yielded back to the caller who is then responsible
    /// for laying out the box.
    ///
    /// They can be used to implement advanced layout modes such as CSS's `float`
    CustomOutOfFlow,
    // machopo: the edges of non-atomic inline boxes.
    /// The start edge of a non-atomic inline box (a CSS `display: inline` element): its start
    /// margin, border and padding, as [`InlineBox::width`].
    ///
    /// Unlike an [`InFlow`](Self::InFlow) box, an edge is not a character of the text: it sits at
    /// a character boundary, like an out-of-flow box, so the text on its two sides is analyzed as
    /// if the edge weren't there. It takes up its width, but adds nothing to the line's height:
    /// the inline's height comes from its content (CSS 2 § 10.8), and its vertical padding and
    /// border don't change the line.
    ///
    /// An edge has no soft wrap opportunity of its own. A start edge stays on the line of the
    /// content after it: an opportunity right before that content is taken before the edge
    /// instead. Consecutive edges (an empty span, or nested span starts) stay together with that
    /// content too.
    StartEdge,
    /// The end edge of a non-atomic inline box, see [`StartEdge`](Self::StartEdge). It stays on
    /// the line of the content before it.
    EndEdge,
}

impl InlineBoxKind {
    /// Whether this is the edge of a non-atomic inline box, which takes up its width on the line.
    pub(crate) fn is_edge(self) -> bool {
        matches!(self, Self::StartEdge | Self::EndEdge)
    }
}
