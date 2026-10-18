# parley (kun's fork)

| | |
| --- | --- |
| Upstream | https://github.com/linebender/parley |
| Fork | https://github.com/wusyong/parley (remote `origin`) |
| Branch | `kun`, from upstream's `97beea381d8afcaddf6b4c625b578e1a6cc60b7a` (main, 2026-09-30) |
| Version | 0.11.0 (unreleased main; the last release is 0.11.1) |
| License | Apache-2.0 OR MIT (`LICENSE-APACHE`, `LICENSE-MIT`, `AUTHORS`) |
| Used by | kun's `crates/layout` (inline layout), `crates/shell` (the address bar: `src/omnibox.rs`, with `PlainEditor`) |

## Why a fork, and why main

- **main, not 0.11.1**: main has `vertical-align` and a baseline for inline
  boxes (upstream #766, 2026-09-28), and sizes a line to contain an inline
  box the way CSS 2 § 10.8 does. In 0.11.1 a 30px box in 20px Ahem text
  gets a 30px line with the box sticking out 2px above it; on main the line
  is 34px. Images need both.
- **Local changes**: inline boxes that are kept with the content next to
  them, and that don't make the line taller (below). Without them, an
  inline element's start or end padding can end up on a different line
  from its text. Upstream lists "Margin/border/padding for inline spans" as
  not done yet in its Blitz tracking issue (#208).

Until 2026-10-01 this was a vendored copy inside kun (`third_party/parley`:
six of the crates, plus the changes as files in `patches/`). It became a
fork, next to kun at `../third_party/parley`, when kun's third-party
projects moved out of the repository; the changes became the commits on
`kun`, and updating upstream became a rebase.

## Local changes

The commits on `kun` after upstream's:

| Commit | What |
| --- | --- |
| Add InlineBox::line_break | `InlineBox::line_break` (`InlineBoxLineBreak`): `Auto` is upstream's behaviour (a soft wrap opportunity on each side of the box); `KeepWithNext` and `KeepWithPrevious` give the box no opportunity of its own and keep it on the line of the content after or before it. For `KeepWithNext`, an opportunity right before that content is taken before the box instead. Both the line breaker and the min/max-content measurer follow this. |
| Don't let inline box edges size the line | A `KeepWithNext` or `KeepWithPrevious` box (an inline's edge) takes up its width but adds nothing to the line's height, as an out-of-flow box doesn't. Upstream counts every in-flow box, so a 0px edge on the baseline made a line taller whenever the text's descent plus half-leading was negative (a `line-height` smaller than the font's), which WPT's `line-height-125` caught. |
| Wrap an atomic inline as the span it is in does | The line breaker and the min/max-content measurer only set the current `text-wrap-mode` from text, so an inline box first on a line (or in a layout) used the default, `Wrap`: in a `nowrap` span the line broke after it (WPT's `css/css-overflow/text-overflow-009.html`). An `Auto` box now takes the mode of its parent style (`parent_style_index`) before its opportunities are decided. A `KeepWithNext` or `KeepWithPrevious` box (an inline's edge) doesn't: the opportunity next to it belongs to the content beside it (WPT's `css/CSS2/linebox/inline-formatting-context-012.xht`). Marked `kun:` in `layout/line_break.rs` and `layout/data.rs`. |
| Give a line break's hanging whitespace | `LineBreakData::hanging_advance`: the part of the line's `advance` that is whitespace hanging past its end, which `commit_line` works out already (`LineMetrics::hanging_advance`). kun breaks the lines beside floats one at a time, each in its own room, and moves a line down to where floats leave more room when its content doesn't fit: the `advance` alone counts the hanging space, so a line that fits looked too wide. Only known once the lines were all broken before, from the lines' metrics, so kun broke them all again; with floats in the lines placed into the block formatting context as each line is final, which can't take a float back, that no longer works. Marked `kun:` in `layout/line_break.rs`. |
| Keep the shaping of layouts alike | `LayoutContext` keeps what shaping a layout's text gave (`shape/cache.rs`, `ShapeCache`), for the next layout with the same text, styled the same in what changes shaping (the font stack, size, width, style, weight, variations, features, locale, letter and word spacing), with its inline boxes at the same places and the same analysis (scripts, breaks, bidi levels); its other styles may differ, and each character and shaped cluster takes the new layout's style index (`ShapedText::set_style_indices` in `parley_engine`). A browser shapes the same text again as content is built again, and alike text in alike elements. Up to 2^18 characters, then it starts again. A kept shaping holds the copy of a font file it was made with (fontique loads a file again once nothing holds its copy, so two copies may be loaded). In a debug build every kept shaping is also shaped again and compared, fonts by their data (`same_shaping`). Marked `kun:` in `shape/mod.rs`, `shape/cache.rs`, `context.rs`, `builder.rs` and `parley_engine`'s `shape/shaped_text.rs`. |
| Make the line after a final break only where asked | `Layout::set_line_after_final_break`: whether the line after a forced break that ends the text (a newline last) is made. By default it is, as upstream has it: an editor puts the caret there. Made, it also counts in the layout's height: `finish_line` commits an empty run on it, so the height's check for an empty last line never sees one. CSS makes no line after the last forced break when nothing follows it (Blink's line breaker stops at the end of the content): `<div>a<br></div>` is one line tall, and Speedometer's Editor-CodeMirror (an empty line is `<div class="cm-line"><br></div>`) laid out one line less. kun makes it only in a text control's inner editor, where Blink adds a placeholder `<br>` to a value ending in a newline. Not made, the line is taken back once committed, before anything is yielded, so a caller breaking lines one at a time never sees it either. Marked `kun:` in `layout/line_break.rs`, `layout/data.rs` and `layout/layout.rs`. |
| Add README.kun.md | This file. |

Their tests are in kun: `cargo test -p layout --test it parley_inline_box::`,
for `hanging_advance` `float::` (the lines beside floats), and for the
kept shapings `shaping::` (and every layout test, in a debug build);
for the line after a final break also `inline::a_final_forced_break_makes_no_line`
and `form_controls::a_text_areas_final_newline_has_a_line_after_it`.
Each part of the changes was removed in turn to check that a test fails.

The vendored copy also trimmed the workspace (no `parley_dev`, upstream's
test harness), so that it loaded on its own: kun's `cargo fmt --all`
formats path dependencies too. With the full tree that isn't needed.

Not proposed upstream yet. Anything sent upstream is drafted first and
reviewed before it goes out.

## Updating

1. `git fetch` upstream (add it as a remote, fetch only) and rebase `kun`
   onto the new upstream commit, fixing any conflicts in the commits above.
2. Update the commit and date above.
3. In kun: `cargo test --workspace` and the reftests. Upstream's API
   changes show up as build errors in `crates/layout/src/text.rs` and
   `crates/shell/src/omnibox.rs`.
