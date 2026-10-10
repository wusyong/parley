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
| Add README.kun.md | This file. |

Their tests are in kun: `cargo test -p layout --test it parley_inline_box::`.
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
