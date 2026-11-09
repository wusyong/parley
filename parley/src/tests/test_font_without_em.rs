// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! machopo: a font without an em is left out of a collection.

use std::{sync::Arc, vec::Vec};

use fontique::{Blob, Collection, CollectionOptions};

/// A test font's data, its `head` table's `unitsPerEm` set to `units_per_em` if given.
fn font_data(units_per_em: Option<u16>) -> Vec<u8> {
    let path = parley_dev::font_dirs()
        .flat_map(|dir| std::fs::read_dir(dir).unwrap())
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "ttf"))
        .unwrap();
    let mut data = std::fs::read(path).unwrap();
    if let Some(units_per_em) = units_per_em {
        // The table directory: 16-byte records after the 12-byte header, of a tag, a checksum,
        // an offset and a length.
        let num_tables = usize::from(u16::from_be_bytes([data[4], data[5]]));
        let record = (0..num_tables)
            .map(|i| 12 + i * 16)
            .find(|&record| &data[record..record + 4] == b"head")
            .unwrap();
        let offset = u32::from_be_bytes(data[record + 8..record + 12].try_into().unwrap());
        // `unitsPerEm` is 18 bytes into `head`.
        let at = offset as usize + 18;
        data[at..at + 2].copy_from_slice(&units_per_em.to_be_bytes());
    }
    data
}

fn registered(data: Vec<u8>) -> usize {
    let mut collection = Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    collection
        .register_fonts(Blob::new(Arc::new(data)), None)
        .iter()
        .map(|(_, fonts)| fonts.len())
        .sum()
}

#[test]
fn a_font_without_an_em_is_left_out() {
    assert_eq!(registered(font_data(None)), 1);
    // Patching the font is not what leaves it out.
    assert_eq!(registered(font_data(Some(1000))), 1);
    assert_eq!(registered(font_data(Some(0))), 0);
}

#[test]
fn a_font_without_a_head_table_is_left_out() {
    let mut data = font_data(None);
    let num_tables = usize::from(u16::from_be_bytes([data[4], data[5]]));
    let record = (0..num_tables)
        .map(|i| 12 + i * 16)
        .find(|&record| &data[record..record + 4] == b"head")
        .unwrap();
    // Another tag in its place: the records stay sorted, unless a font has a `heb` table.
    data[record..record + 4].copy_from_slice(b"hea_");
    assert_eq!(registered(data), 0);
}
