// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! kun: how far shaping with a font reaches across a space, so that text
//! in it shapes the same a word at a time as all at once.

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use parlance::FontFeature;
use skrifa::MetadataProvider;
use skrifa::raw::tables::gpos::{PairPos, PositionLookup, PositionSubtables, ValueRecord};
use skrifa::raw::tables::gsub::{
    SingleSubst, SubstitutionLookup, SubstitutionLookupList, SubstitutionSubtables,
};
use skrifa::raw::tables::kern::SubtableKind;
use skrifa::raw::tables::layout::{
    ChainedSequenceContext, ClassDef, CoverageTable, Feature, FeatureList, FeatureVariations,
    LookupFlag, SequenceContext, SequenceLookupRecord,
};
use skrifa::raw::types::{BigEndian, GlyphId, GlyphId16, Tag};
use skrifa::raw::{FontRef, ReadError, ResolveOffset, TableProvider};

/// The most glyphs, but marks, beside a space on either side that shaping
/// takes together with it, for text in the font to be shaped a word at a
/// time: with more, most words would be shaped again with their neighbours.
const MAX_BESIDE: usize = 256;

/// How far shaping with a font reaches across a space.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SpaceReach {
    /// Not at all: a word shapes on its own as within the text around it.
    Apart,
    /// Only to the glyph beside the space: a word shapes on its own as
    /// within the text around it but where a space and a glyph beside it
    /// are [`Beside`]'s.
    Beside(Beside),
    /// Further, or in a way not looked into: text in the font is shaped
    /// whole.
    Across,
}

/// The glyphs shaping takes together with a space beside them.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Beside {
    /// What positioning takes: only where the glyph or the space goes
    /// changes, so the word beside the space is shaped again with it.
    pub(crate) moved: Sides,
    /// What substitutions take: the space may become another glyph, which
    /// may be shaped with the word on its other side too, so the words on
    /// both sides are shaped again with it.
    pub(crate) substituted: Sides,
    /// Whether a lookup taking a glyph and the space after it skips the
    /// marks between them: a word ending in a grapheme of several
    /// characters is then shaped again with the words on both sides of the
    /// space after it (without GDEF, harfrust tells marks by their
    /// characters).
    pub(crate) skips_marks: bool,
    /// Whether a ligature may take a space and what follows it: a space
    /// with the rest of its grapheme is then shaped again with the words on
    /// both sides, as what the ligature makes of it isn't looked into.
    pub(crate) space_ligatures: bool,
}

/// The glyphs taken with a space after them, and with one before them.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Sides {
    /// Sorted.
    before: Box<[u16]>,
    /// Sorted.
    after: Box<[u16]>,
}

impl Sides {
    /// Whether shaping may take `glyph` together with a space after it.
    pub(crate) fn before(&self, glyph: u32) -> bool {
        u16::try_from(glyph).map_or(true, |glyph| self.before.binary_search(&glyph).is_ok())
    }

    /// Whether shaping may take `glyph` together with a space before it.
    pub(crate) fn after(&self, glyph: u32) -> bool {
        u16::try_from(glyph).map_or(true, |glyph| self.after.binary_search(&glyph).is_ok())
    }
}

/// How far shaping with `font` reaches across a space: whether a word
/// shaped on its own can come out other than within the text around it,
/// and where. Words are split before a space and after one, never inside a
/// grapheme, so a space's own combining marks stay with it.
///
/// Shaping takes a space together with other glyphs when a lookup that
/// may apply matches the space with the glyph before it, or with one after
/// it that can be other than a combining mark: a ligature, a kerning pair,
/// or a context with the space in it. A rule of two glyphs, the space and
/// one beside it, reaches only that glyph ([`SpaceReach::Beside`]): those
/// glyphs, with what substitutions after the rule's may make of them, are
/// told apart, a substitution's apart from a positioning's. What a
/// substitution makes of the space where its rule matches may be shaped
/// with the glyph on its other side too; a rule taking that glyph with
/// others can't reach further than the words on both sides without taking
/// a space, which is looked into too. A longer rule reaches across. A
/// lookup that only changes the space itself (a single substitution, a
/// single adjustment) doesn't reach, but what it makes of the space is a
/// space too; a ligature starting with the space, which may take the rest
/// of its grapheme, makes the grapheme shape with the words on both sides
/// ([`Beside::space_ligatures`]). Neither do the lookups only features
/// that are off unless asked for reach (`dlig`, `ss01`, …:
/// [`asks_for_rare_features`]); every other feature counts, as do a `kern`
/// table's pairs, while a lookup that skips base glyphs (it can match
/// across one), shaping by AAT tables (which harfrust then uses) and tables
/// that can't be read reach across. Gecko's
/// `gfxFont::CheckForFeaturesInvolvingSpace` tells the like for the whole
/// font, not the glyphs beside a space, through `collect_glyphs` in
/// `HarfBuzz`, which harfrust lacks; it then still shapes a word on its own
/// when only kerning involves a space, kerning the space otherwise than
/// shaping the text whole does, where words shaped again with it here
/// don't.
///
/// A font without a space glyph never shapes one: a space then takes
/// another font, which splits the shaping there anyway.
pub(crate) fn space_reach(font: &FontRef<'_>) -> SpaceReach {
    let Some(space) = font.charmap().map(' ') else {
        return SpaceReach::Apart;
    };
    let Ok(space) = u16::try_from(space.to_u32()).map(GlyphId16::new) else {
        return SpaceReach::Across;
    };
    if [b"morx", b"mort", b"kerx"]
        .iter()
        .any(|tag| font.table_data(Tag::new(tag)).is_some())
    {
        return SpaceReach::Across;
    }
    reach(font, space).unwrap_or(SpaceReach::Across)
}

/// Whether `features` turn on a feature that is off unless asked for: its
/// lookups aren't looked into by [`space_reach`].
pub(crate) fn asks_for_rare_features(features: &[FontFeature]) -> bool {
    features
        .iter()
        .any(|feature| feature.value != 0 && off_by_default(feature.tag.to_bytes()))
}

/// Features `HarfBuzz` applies only when asked for: the registered ones that
/// aren't on for any script or by default (character variants and
/// stylistic sets included). Any other counts as on.
fn off_by_default(tag: [u8; 4]) -> bool {
    const OFF: &[&[u8; 4]] = &[
        b"aalt", b"afrc", b"c2pc", b"c2sc", b"case", b"cpct", b"cpsp", b"cswh", b"dlig", b"dnom",
        b"expt", b"falt", b"frac", b"fwid", b"halt", b"hist", b"hkna", b"hlig", b"hngl", b"hojo",
        b"hwid", b"ital", b"jalt", b"jp04", b"jp78", b"jp83", b"jp90", b"lfbd", b"lnum", b"mgrk",
        b"nalt", b"nlck", b"numr", b"onum", b"opbd", b"ordn", b"ornm", b"palt", b"pcap", b"pkna",
        b"pnum", b"pwid", b"qwid", b"rtbd", b"ruby", b"salt", b"sinf", b"smcp", b"smpl", b"subs",
        b"sups", b"swsh", b"titl", b"tnam", b"tnum", b"trad", b"twid", b"unic", b"valt", b"vhal",
        b"vkna", b"vkrn", b"vpal", b"vrt2", b"vrtr", b"vert", b"zero",
    ];
    let digits = tag[2].is_ascii_digit() && tag[3].is_ascii_digit();
    OFF.contains(&&tag) || (digits && (&tag[..2] == b"ss" || &tag[..2] == b"cv"))
}

/// [`space_reach`], an error where a table can't be read.
fn reach(font: &FontRef<'_>, space: GlyphId16) -> Result<SpaceReach, ReadError> {
    let marks = match font.gdef() {
        Ok(gdef) => gdef.glyph_class_def().transpose()?,
        Err(ReadError::TableIsMissing(_)) => None,
        Err(error) => return Err(error),
    };
    let gsub = match font.gsub() {
        Ok(gsub) => {
            let lookups = gsub.lookup_list()?;
            let on = lookups_on(
                &gsub.feature_list()?,
                gsub.feature_variations().transpose()?,
                lookups.lookups().len(),
                |index| nested_in_substitution(&lookups.lookups().get(index)?),
            )?;
            Some((lookups, on))
        }
        Err(ReadError::TableIsMissing(_)) => None,
        Err(error) => return Err(error),
    };

    // What the space may become is a space too: what a context makes of it,
    // where the context matches, is a rule taking the space.
    let mut spaces = GlyphSet::default();
    spaces.insert(space);
    if let Some((lookups, on)) = &gsub {
        substituted(lookups, on, true, &mut spaces)?;
    }
    let font_space = FontSpace {
        spaces: spaces.iter().collect(),
        marks,
    };

    // Substitutions see a glyph before what later ones make of it;
    // positioning, and the `kern` table, after.
    let mut by_substitution = Reached::default();
    let mut by_position = Reached::default();
    font_space.in_kern(font, &mut by_position)?;
    if let Some((lookups, on)) = &gsub {
        for (index, applied) in on.iter().enumerate() {
            if *applied != Applied::Not {
                font_space.substitution(&lookups.lookups().get(index)?, &mut by_substitution)?;
                if by_substitution.across {
                    return Ok(SpaceReach::Across);
                }
            }
        }
    }
    match font.gpos() {
        Ok(gpos) => {
            let lookups = gpos.lookup_list()?.lookups();
            let on = lookups_on(
                &gpos.feature_list()?,
                gpos.feature_variations().transpose()?,
                lookups.len(),
                |index| nested_in_position(&lookups.get(index)?),
            )?;
            for (index, applied) in on.into_iter().enumerate() {
                if applied != Applied::Not {
                    font_space.position(&lookups.get(index)?, &mut by_position)?;
                    if by_position.across {
                        return Ok(SpaceReach::Across);
                    }
                }
            }
        }
        Err(ReadError::TableIsMissing(_)) => {}
        Err(error) => return Err(error),
    }
    if by_position.across {
        return Ok(SpaceReach::Across);
    }

    if let Some((lookups, on)) = &gsub {
        substituted(lookups, on, false, &mut by_substitution.before)?;
        substituted(lookups, on, false, &mut by_substitution.after)?;
    }
    let space_ligatures = by_substitution.space_ligatures;
    if [&by_position, &by_substitution]
        .iter()
        .all(|reached| reached.before.is_empty() && reached.after.is_empty())
        && !space_ligatures
    {
        return Ok(SpaceReach::Apart);
    }
    let count = |a: &GlyphSet, b: &GlyphSet| {
        let mut set = GlyphSet::default();
        set.extend(a);
        set.extend(b);
        set.iter()
            .filter(|glyph| !font_space.is_mark(*glyph))
            .count()
    };
    if count(&by_position.before, &by_substitution.before) > MAX_BESIDE
        || count(&by_position.after, &by_substitution.after) > MAX_BESIDE
    {
        return Ok(SpaceReach::Across);
    }
    // A mark ending a word may hide the glyph before it from a lookup that
    // skips marks; one a substitution made from a single character, too.
    for reached in [&mut by_position, &mut by_substitution] {
        if reached.before_skips_marks
            && let Some(marks) = &font_space.marks
        {
            for (glyph, class) in marks.iter() {
                if class == 3 {
                    reached.before.insert(glyph);
                }
            }
        }
    }
    let sides = |reached: &Reached| Sides {
        before: reached.before.iter().map(GlyphId16::to_u16).collect(),
        after: reached.after.iter().map(GlyphId16::to_u16).collect(),
    };
    Ok(SpaceReach::Beside(Beside {
        moved: sides(&by_position),
        substituted: sides(&by_substitution),
        skips_marks: by_position.before_skips_marks || by_substitution.before_skips_marks,
        space_ligatures,
    }))
}

/// A set of glyphs.
#[derive(Default)]
struct GlyphSet {
    bits: Vec<u64>,
    len: usize,
}

impl GlyphSet {
    fn insert(&mut self, glyph: GlyphId16) -> bool {
        if self.bits.is_empty() {
            self.bits = vec![0; 1 << 10];
        }
        let glyph = usize::from(glyph.to_u16());
        let (word, bit) = (&mut self.bits[glyph / 64], 1_u64 << (glyph % 64));
        let added = *word & bit == 0;
        *word |= bit;
        self.len += usize::from(added);
        added
    }

    fn contains(&self, glyph: GlyphId16) -> bool {
        let glyph = usize::from(glyph.to_u16());
        self.bits
            .get(glyph / 64)
            .is_some_and(|word| word & (1_u64 << (glyph % 64)) != 0)
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn extend(&mut self, other: &Self) {
        for glyph in other.iter() {
            self.insert(glyph);
        }
    }

    /// The glyphs, in order.
    fn iter(&self) -> impl Iterator<Item = GlyphId16> + '_ {
        self.bits.iter().enumerate().flat_map(|(index, &word)| {
            (0..64_usize)
                .filter(move |bit| word & (1_u64 << bit) != 0)
                .filter_map(move |bit| u16::try_from(index * 64 + bit).ok().map(GlyphId16::new))
        })
    }
}

/// Adds to `set` what the substitution lookups that may apply (`applied`)
/// may make of its glyphs, again and again, and the ligatures they may be
/// part of; `of_space`, what the lookups of features make of a space alone,
/// without ligatures.
fn substituted(
    lookups: &SubstitutionLookupList<'_>,
    applied: &[Applied],
    of_space: bool,
    set: &mut GlyphSet,
) -> Result<(), ReadError> {
    if set.is_empty() {
        return Ok(());
    }
    loop {
        let mut added = false;
        for (index, applied) in applied.iter().enumerate() {
            if *applied == Applied::Not || (of_space && *applied != Applied::ByFeature) {
                continue;
            }
            match lookups.lookups().get(index)?.subtables()? {
                SubstitutionSubtables::Single(subtables) => {
                    for table in subtables.iter() {
                        match table? {
                            SingleSubst::Format1(table) => {
                                let delta = table.delta_glyph_id();
                                for glyph in table.coverage()?.iter() {
                                    if set.contains(glyph) {
                                        // Modulo 2^16, as the format says.
                                        let to = glyph.to_u16().wrapping_add_signed(delta);
                                        added |= set.insert(GlyphId16::new(to));
                                    }
                                }
                            }
                            SingleSubst::Format2(table) => {
                                let to = table.substitute_glyph_ids();
                                for (glyph, to) in table.coverage()?.iter().zip(to) {
                                    if set.contains(glyph) {
                                        added |= set.insert(to.get());
                                    }
                                }
                            }
                        }
                    }
                }
                SubstitutionSubtables::Multiple(subtables) => {
                    for table in subtables.iter() {
                        let table = table?;
                        for (glyph, sequence) in
                            table.coverage()?.iter().zip(table.sequences().iter())
                        {
                            if set.contains(glyph) {
                                for to in sequence?.substitute_glyph_ids() {
                                    added |= set.insert(to.get());
                                }
                            }
                        }
                    }
                }
                SubstitutionSubtables::Alternate(subtables) => {
                    for table in subtables.iter() {
                        let table = table?;
                        let sets = table.alternate_sets();
                        for (glyph, alternates) in table.coverage()?.iter().zip(sets.iter()) {
                            if set.contains(glyph) {
                                for to in alternates?.alternate_glyph_ids() {
                                    added |= set.insert(to.get());
                                }
                            }
                        }
                    }
                }
                SubstitutionSubtables::Ligature(subtables) if !of_space => {
                    for table in subtables.iter() {
                        let table = table?;
                        let sets = table.ligature_sets();
                        for (first, ligatures) in table.coverage()?.iter().zip(sets.iter()) {
                            for ligature in ligatures?.ligatures().iter() {
                                let ligature = ligature?;
                                if set.contains(first)
                                    || ligature
                                        .component_glyph_ids()
                                        .iter()
                                        .any(|glyph| set.contains(glyph.get()))
                                {
                                    added |= set.insert(ligature.ligature_glyph());
                                }
                            }
                        }
                    }
                }
                SubstitutionSubtables::Reverse(subtables) => {
                    for table in subtables.iter() {
                        let table = table?;
                        let to = table.substitute_glyph_ids();
                        for (glyph, to) in table.coverage()?.iter().zip(to) {
                            if set.contains(glyph) {
                                added |= set.insert(to.get());
                            }
                        }
                    }
                }
                // Contexts substitute through other lookups, which are on too.
                SubstitutionSubtables::Ligature(_)
                | SubstitutionSubtables::Contextual(_)
                | SubstitutionSubtables::ChainContextual(_)
                | SubstitutionSubtables::EmptyExtension => {}
            }
        }
        if !added {
            return Ok(());
        }
    }
}

/// What the lookups looked at reach across a space.
#[derive(Default)]
struct Reached {
    /// Whether they reach further than [`Beside`] tells.
    across: bool,
    before: GlyphSet,
    after: GlyphSet,
    before_skips_marks: bool,
    space_ligatures: bool,
}

/// A font's space glyphs, and what tells its combining marks.
struct FontSpace<'a> {
    /// The space and what substitutions may make of it.
    spaces: Vec<GlyphId16>,
    /// GDEF's glyph classes; without them, no glyph counts as a mark (harfrust
    /// then takes them from the characters, which a glyph alone doesn't tell).
    marks: Option<ClassDef<'a>>,
}

/// One glyph of a rule: what may match there.
enum Position<'a> {
    Glyph(GlyphId16),
    /// A class of a class definition, and the coverage the glyph is also
    /// in, if any.
    Class(&'a ClassDef<'a>, u16, Option<&'a CoverageTable<'a>>),
    Coverage(CoverageTable<'a>),
}

impl FontSpace<'_> {
    fn is_mark(&self, glyph: GlyphId16) -> bool {
        self.marks
            .as_ref()
            .is_some_and(|marks| marks.get(GlyphId::from(glyph)) == 3)
    }

    fn is_space(&self, glyph: GlyphId16) -> bool {
        self.spaces.contains(&glyph)
    }

    fn may_be_space(&self, position: &Position<'_>) -> bool {
        self.spaces.iter().any(|&space| match position {
            Position::Glyph(glyph) => *glyph == space,
            Position::Class(class_def, class, coverage) => {
                class_def.get(GlyphId::from(space)) == *class
                    && coverage.is_none_or(|coverage| coverage.get(space).is_some())
            }
            Position::Coverage(coverage) => coverage.get(space).is_some(),
        })
    }

    /// Whether only combining marks match at `position`.
    fn only_marks(&self, position: &Position<'_>) -> bool {
        match position {
            Position::Glyph(glyph) => self.is_mark(*glyph),
            // Class 0 is every glyph the classes leave out.
            Position::Class(class_def, class, _) => {
                *class != 0
                    && class_def
                        .iter()
                        .filter(|(_, c)| c == class)
                        .all(|(glyph, _)| self.is_mark(glyph))
            }
            Position::Coverage(coverage) => coverage.iter().all(|glyph| self.is_mark(glyph)),
        }
    }

    /// Adds to `reached` how a rule matching `positions` in order (the
    /// backtrack's from the farthest) reaches across a space: where it may
    /// take a space with the glyph before it, or with one after it that
    /// isn't a combining mark (any glyph, if the lookup skips marks).
    fn rule(&self, positions: &[Position<'_>], flag: LookupFlag, reached: &mut Reached) {
        let skips_marks = flag.contains(LookupFlag::IGNORE_MARKS)
            || flag.contains(LookupFlag::USE_MARK_FILTERING_SET)
            || flag.mark_attachment_class().is_some();
        for (index, position) in positions.iter().enumerate() {
            if !self.may_be_space(position) {
                continue;
            }
            let next = positions.get(index + 1);
            if index == 0 && !next.is_some_and(|next| skips_marks || !self.only_marks(next)) {
                continue;
            }
            let [first, second] = positions else {
                reached.across = true;
                return;
            };
            let added = if index == 0 {
                self.add_glyphs(second, &mut reached.after)
            } else {
                reached.before_skips_marks |= skips_marks;
                self.add_glyphs(first, &mut reached.before)
            };
            if !added {
                reached.across = true;
                return;
            }
        }
    }

    /// Adds the glyphs that may match at `position` to `set`; `false` if
    /// they are every glyph but some.
    fn add_glyphs(&self, position: &Position<'_>, set: &mut GlyphSet) -> bool {
        match position {
            Position::Glyph(glyph) => {
                set.insert(*glyph);
            }
            Position::Class(class_def, class, coverage) => match (*class, coverage) {
                (0, None) => return false,
                (0, Some(coverage)) => {
                    for glyph in coverage.iter() {
                        if class_def.get(GlyphId::from(glyph)) == 0 {
                            set.insert(glyph);
                        }
                    }
                }
                (class, coverage) => {
                    for (glyph, _) in class_def.iter().filter(|&(_, c)| c == class) {
                        if coverage.is_none_or(|coverage| coverage.get(glyph).is_some()) {
                            set.insert(glyph);
                        }
                    }
                }
            },
            Position::Coverage(coverage) => {
                for glyph in coverage.iter() {
                    set.insert(glyph);
                }
            }
        }
        true
    }

    fn in_kern(&self, font: &FontRef<'_>, reached: &mut Reached) -> Result<(), ReadError> {
        let kern = match font.kern() {
            Ok(kern) => kern,
            Err(ReadError::TableIsMissing(_)) => return Ok(()),
            Err(error) => return Err(error),
        };
        for subtable in kern.subtables() {
            match subtable?.kind()? {
                SubtableKind::Format0(pairs) => {
                    for pair in pairs.pairs() {
                        if self.is_space(pair.right()) {
                            // HarfBuzz kerns past marks.
                            reached.before_skips_marks = true;
                            reached.before.insert(pair.left());
                        }
                        if self.is_space(pair.left()) && !self.is_mark(pair.right()) {
                            reached.after.insert(pair.right());
                        }
                    }
                }
                // Classes and state machines: rare; not looked into.
                _ => reached.across = true,
            }
        }
        Ok(())
    }

    fn substitution(
        &self,
        lookup: &SubstitutionLookup<'_>,
        reached: &mut Reached,
    ) -> Result<(), ReadError> {
        let flag = lookup.lookup_flag();
        if skips_bases(flag) {
            reached.across = true;
            return Ok(());
        }
        match lookup.subtables()? {
            // These change the space alone.
            SubstitutionSubtables::Single(_)
            | SubstitutionSubtables::Multiple(_)
            | SubstitutionSubtables::Alternate(_)
            | SubstitutionSubtables::EmptyExtension => {}
            SubstitutionSubtables::Ligature(subtables) => {
                for table in subtables.iter() {
                    let table = table?;
                    let coverage = table.coverage()?;
                    for (first, set) in coverage.iter().zip(table.ligature_sets().iter()) {
                        reached.space_ligatures |= self.is_space(first);
                        for ligature in set?.ligatures().iter() {
                            let positions: Vec<Position<'_>> = core::iter::once(first)
                                .chain(ligature?.component_glyph_ids().iter().map(BigEndian::get))
                                .map(Position::Glyph)
                                .collect();
                            self.rule(&positions, flag, reached);
                        }
                    }
                }
            }
            SubstitutionSubtables::Contextual(subtables) => {
                for context in subtables.iter() {
                    self.context(&context?, flag, reached)?;
                }
            }
            SubstitutionSubtables::ChainContextual(subtables) => {
                for context in subtables.iter() {
                    self.chained(&context?, flag, reached)?;
                }
            }
            SubstitutionSubtables::Reverse(subtables) => {
                for table in subtables.iter() {
                    let table = table?;
                    let mut positions: Vec<Position<'_>> = table
                        .backtrack_coverages()
                        .iter()
                        .map(|coverage| coverage.map(Position::Coverage))
                        .collect::<Result<_, _>>()?;
                    positions.reverse();
                    positions.push(Position::Coverage(table.coverage()?));
                    for coverage in table.lookahead_coverages().iter() {
                        positions.push(Position::Coverage(coverage?));
                    }
                    self.rule(&positions, flag, reached);
                }
            }
        }
        Ok(())
    }

    fn position(
        &self,
        lookup: &PositionLookup<'_>,
        reached: &mut Reached,
    ) -> Result<(), ReadError> {
        let flag = lookup.lookup_flag();
        if skips_bases(flag) {
            reached.across = true;
            return Ok(());
        }
        let has_space = |coverage: CoverageTable<'_>| {
            self.spaces
                .iter()
                .any(|&space| coverage.get(space).is_some())
        };
        match lookup.subtables()? {
            // Moves the space alone.
            PositionSubtables::Single(_) | PositionSubtables::EmptyExtension => {}
            PositionSubtables::Pair(subtables) => {
                for table in subtables.iter() {
                    match table? {
                        PairPos::Format1(table) => {
                            let coverage = table.coverage()?;
                            for (first, set) in coverage.iter().zip(table.pair_sets().iter()) {
                                for record in set?.pair_value_records().iter() {
                                    let positions = [
                                        Position::Glyph(first),
                                        Position::Glyph(record?.second_glyph()),
                                    ];
                                    self.rule(&positions, flag, reached);
                                }
                            }
                        }
                        PairPos::Format2(table) => {
                            let coverage = table.coverage()?;
                            let (class_def1, class_def2) =
                                (table.class_def1()?, table.class_def2()?);
                            for (class1, record) in table.class1_records().iter().enumerate() {
                                let record = record?;
                                for (class2, pair) in record.class2_records().iter().enumerate() {
                                    let pair = pair?;
                                    if !moves(pair.value_record1()) && !moves(pair.value_record2())
                                    {
                                        continue;
                                    }
                                    let (Ok(class1), Ok(class2)) =
                                        (u16::try_from(class1), u16::try_from(class2))
                                    else {
                                        reached.across = true;
                                        return Ok(());
                                    };
                                    let positions = [
                                        Position::Class(&class_def1, class1, Some(&coverage)),
                                        Position::Class(&class_def2, class2, None),
                                    ];
                                    self.rule(&positions, flag, reached);
                                }
                            }
                        }
                    }
                }
            }
            // Joins the space to the glyphs beside it.
            PositionSubtables::Cursive(subtables) => {
                for table in subtables.iter() {
                    reached.across |= has_space(table?.coverage()?);
                }
            }
            // Marks on a space are its own (in its grapheme); a space as a
            // mark is odd enough to count.
            PositionSubtables::MarkToBase(subtables) => {
                for table in subtables.iter() {
                    reached.across |= has_space(table?.mark_coverage()?);
                }
            }
            PositionSubtables::MarkToLig(subtables) => {
                for table in subtables.iter() {
                    reached.across |= has_space(table?.mark_coverage()?);
                }
            }
            PositionSubtables::MarkToMark(subtables) => {
                for table in subtables.iter() {
                    let table = table?;
                    reached.across |=
                        has_space(table.mark1_coverage()?) || has_space(table.mark2_coverage()?);
                }
            }
            PositionSubtables::Contextual(subtables) => {
                for context in subtables.iter() {
                    self.context(&context?, flag, reached)?;
                }
            }
            PositionSubtables::ChainContextual(subtables) => {
                for context in subtables.iter() {
                    self.chained(&context?, flag, reached)?;
                }
            }
        }
        Ok(())
    }

    fn context(
        &self,
        context: &SequenceContext<'_>,
        flag: LookupFlag,
        reached: &mut Reached,
    ) -> Result<(), ReadError> {
        match context {
            SequenceContext::Format1(table) => {
                let coverage = table.coverage()?;
                for (first, set) in coverage.iter().zip(table.seq_rule_sets().iter()) {
                    let Some(set) = set else { continue };
                    for rule in set?.seq_rules().iter() {
                        let positions: Vec<Position<'_>> = core::iter::once(first)
                            .chain(rule?.input_sequence().iter().map(BigEndian::get))
                            .map(Position::Glyph)
                            .collect();
                        self.rule(&positions, flag, reached);
                    }
                }
            }
            SequenceContext::Format2(table) => {
                let coverage = table.coverage()?;
                let class_def = table.class_def()?;
                for (first, set) in table.class_seq_rule_sets().iter().enumerate() {
                    let Some(set) = set else { continue };
                    let Ok(first) = u16::try_from(first) else {
                        reached.across = true;
                        return Ok(());
                    };
                    for rule in set?.class_seq_rules().iter() {
                        let positions: Vec<Position<'_>> =
                            core::iter::once(Position::Class(&class_def, first, Some(&coverage)))
                                .chain(
                                    rule?.input_sequence().iter().map(|class| {
                                        Position::Class(&class_def, class.get(), None)
                                    }),
                                )
                                .collect();
                        self.rule(&positions, flag, reached);
                    }
                }
            }
            SequenceContext::Format3(table) => {
                let positions: Vec<Position<'_>> = table
                    .coverages()
                    .iter()
                    .map(|coverage| coverage.map(Position::Coverage))
                    .collect::<Result<_, _>>()?;
                self.rule(&positions, flag, reached);
            }
        }
        Ok(())
    }

    fn chained(
        &self,
        context: &ChainedSequenceContext<'_>,
        flag: LookupFlag,
        reached: &mut Reached,
    ) -> Result<(), ReadError> {
        match context {
            ChainedSequenceContext::Format1(table) => {
                let coverage = table.coverage()?;
                for (first, set) in coverage.iter().zip(table.chained_seq_rule_sets().iter()) {
                    let Some(set) = set else { continue };
                    for rule in set?.chained_seq_rules().iter() {
                        let rule = rule?;
                        let positions: Vec<Position<'_>> = rule
                            .backtrack_sequence()
                            .iter()
                            .rev()
                            .map(BigEndian::get)
                            .chain([first])
                            .chain(rule.input_sequence().iter().map(BigEndian::get))
                            .chain(rule.lookahead_sequence().iter().map(BigEndian::get))
                            .map(Position::Glyph)
                            .collect();
                        self.rule(&positions, flag, reached);
                    }
                }
            }
            ChainedSequenceContext::Format2(table) => {
                let coverage = table.coverage()?;
                let backtrack = table.backtrack_class_def()?;
                let input = table.input_class_def()?;
                let lookahead = table.lookahead_class_def()?;
                for (first, set) in table.chained_class_seq_rule_sets().iter().enumerate() {
                    let Some(set) = set else { continue };
                    let Ok(first) = u16::try_from(first) else {
                        reached.across = true;
                        return Ok(());
                    };
                    for rule in set?.chained_class_seq_rules().iter() {
                        let rule = rule?;
                        let positions: Vec<Position<'_>> = rule
                            .backtrack_sequence()
                            .iter()
                            .rev()
                            .map(|class| Position::Class(&backtrack, class.get(), None))
                            .chain([Position::Class(&input, first, Some(&coverage))])
                            .chain(
                                rule.input_sequence()
                                    .iter()
                                    .map(|class| Position::Class(&input, class.get(), None)),
                            )
                            .chain(
                                rule.lookahead_sequence()
                                    .iter()
                                    .map(|class| Position::Class(&lookahead, class.get(), None)),
                            )
                            .collect();
                        self.rule(&positions, flag, reached);
                    }
                }
            }
            ChainedSequenceContext::Format3(table) => {
                let mut positions: Vec<Position<'_>> = table
                    .backtrack_coverages()
                    .iter()
                    .map(|coverage| coverage.map(Position::Coverage))
                    .collect::<Result<_, _>>()?;
                positions.reverse();
                for coverages in [table.input_coverages(), table.lookahead_coverages()] {
                    for coverage in coverages.iter() {
                        positions.push(Position::Coverage(coverage?));
                    }
                }
                self.rule(&positions, flag, reached);
            }
        }
        Ok(())
    }
}

/// How a lookup may apply.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Applied {
    Not,
    /// Only where a context that applies it matches.
    InContext,
    /// Wherever its feature is on.
    ByFeature,
}

/// How each of `count` lookups may apply: those of the features that
/// aren't off by default (feature variations' too) by them, and those
/// their contexts apply (`nested`, transitively) in context.
fn lookups_on(
    features: &FeatureList<'_>,
    variations: Option<FeatureVariations<'_>>,
    count: usize,
    mut nested: impl FnMut(usize) -> Result<Vec<u16>, ReadError>,
) -> Result<Vec<Applied>, ReadError> {
    let mut applied = vec![Applied::Not; count];
    let mut pending: Vec<u16> = Vec::new();
    let records = features.feature_records();
    for record in records {
        if !off_by_default(record.feature_tag().to_be_bytes()) {
            let feature = record.feature(features.offset_data())?;
            pending.extend(feature.lookup_list_indices().iter().map(BigEndian::get));
        }
    }
    if let Some(variations) = variations {
        for record in variations.feature_variation_records() {
            let Some(substitution) = record.feature_table_substitution(variations.offset_data())
            else {
                continue;
            };
            let substitution = substitution?;
            for alternate in substitution.substitutions() {
                let tag = records
                    .get(usize::from(alternate.feature_index()))
                    .map_or(Tag::new(&[0; 4]), |record| record.feature_tag());
                if off_by_default(tag.to_be_bytes()) {
                    continue;
                }
                let feature: Feature<'_> = alternate
                    .alternate_feature_offset()
                    .resolve_with_args(substitution.offset_data(), &tag)?;
                pending.extend(feature.lookup_list_indices().iter().map(BigEndian::get));
            }
        }
    }
    for &index in &pending {
        *applied
            .get_mut(usize::from(index))
            .ok_or(ReadError::OutOfBounds)? = Applied::ByFeature;
    }
    let mut seen = vec![false; count];
    while let Some(index) = pending.pop() {
        let index = usize::from(index);
        match seen.get_mut(index) {
            Some(seen) if !*seen => *seen = true,
            Some(_) => continue,
            None => return Err(ReadError::OutOfBounds),
        }
        if applied[index] == Applied::Not {
            applied[index] = Applied::InContext;
        }
        pending.extend(nested(index)?);
    }
    Ok(applied)
}

fn nested_in_substitution(lookup: &SubstitutionLookup<'_>) -> Result<Vec<u16>, ReadError> {
    let mut nested = Vec::new();
    match lookup.subtables()? {
        SubstitutionSubtables::Contextual(subtables) => {
            for context in subtables.iter() {
                context_lookups(&context?, &mut nested)?;
            }
        }
        SubstitutionSubtables::ChainContextual(subtables) => {
            for context in subtables.iter() {
                chained_lookups(&context?, &mut nested)?;
            }
        }
        _ => {}
    }
    Ok(nested)
}

fn nested_in_position(lookup: &PositionLookup<'_>) -> Result<Vec<u16>, ReadError> {
    let mut nested = Vec::new();
    match lookup.subtables()? {
        PositionSubtables::Contextual(subtables) => {
            for context in subtables.iter() {
                context_lookups(&context?, &mut nested)?;
            }
        }
        PositionSubtables::ChainContextual(subtables) => {
            for context in subtables.iter() {
                chained_lookups(&context?, &mut nested)?;
            }
        }
        _ => {}
    }
    Ok(nested)
}

/// Adds the lookups `records` apply to `nested`.
fn add_records(records: &[SequenceLookupRecord], nested: &mut Vec<u16>) {
    nested.extend(records.iter().map(SequenceLookupRecord::lookup_list_index));
}

/// The lookups a context applies, onto `nested`.
fn context_lookups(context: &SequenceContext<'_>, nested: &mut Vec<u16>) -> Result<(), ReadError> {
    match context {
        SequenceContext::Format1(table) => {
            for set in table.seq_rule_sets().iter().flatten() {
                for rule in set?.seq_rules().iter() {
                    add_records(rule?.seq_lookup_records(), nested);
                }
            }
        }
        SequenceContext::Format2(table) => {
            for set in table.class_seq_rule_sets().iter().flatten() {
                for rule in set?.class_seq_rules().iter() {
                    add_records(rule?.seq_lookup_records(), nested);
                }
            }
        }
        SequenceContext::Format3(table) => add_records(table.seq_lookup_records(), nested),
    }
    Ok(())
}

fn chained_lookups(
    context: &ChainedSequenceContext<'_>,
    nested: &mut Vec<u16>,
) -> Result<(), ReadError> {
    match context {
        ChainedSequenceContext::Format1(table) => {
            for set in table.chained_seq_rule_sets().iter().flatten() {
                for rule in set?.chained_seq_rules().iter() {
                    add_records(rule?.seq_lookup_records(), nested);
                }
            }
        }
        ChainedSequenceContext::Format2(table) => {
            for set in table.chained_class_seq_rule_sets().iter().flatten() {
                for rule in set?.chained_class_seq_rules().iter() {
                    add_records(rule?.seq_lookup_records(), nested);
                }
            }
        }
        ChainedSequenceContext::Format3(table) => add_records(table.seq_lookup_records(), nested),
    }
    Ok(())
}

/// Whether a lookup with `flag` skips base glyphs, a space among them.
fn skips_bases(flag: LookupFlag) -> bool {
    flag.contains(LookupFlag::IGNORE_BASE_GLYPHS)
}

/// Whether `value` moves a glyph or changes its advance.
fn moves(value: &ValueRecord) -> bool {
    [
        value.x_placement,
        value.y_placement,
        value.x_advance,
        value.y_advance,
    ]
    .iter()
    .any(|v| v.is_some_and(|v| v.get() != 0))
        || [
            value.x_placement_device,
            value.y_placement_device,
            value.x_advance_device,
            value.y_advance_device,
        ]
        .iter()
        .any(|device| !device.get().is_null())
}
