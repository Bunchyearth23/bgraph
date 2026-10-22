//! Cutting a TrueType font down to the glyphs a diagram uses.
//!
//! # The shape of the compromise
//!
//! A *real* subsetter renumbers the glyphs and rebuilds `cmap`, `hmtx`, `post`
//! and the layout tables to match. That is a project, and a subtly wrong font
//! fails at the reader's end with no message — the worst possible failure for
//! something whose whole job is to make the reader's view match the author's.
//!
//! So this does the conservative half: **every glyph keeps its id**, and the
//! ones the diagram never uses have their outlines emptied. Only `glyf` and
//! `loca` change; `cmap`, `hmtx`, `head`, `hhea`, `maxp` and the rest are
//! copied through untouched, so nothing can point at the wrong glyph because
//! nothing was renumbered.
//!
//! `glyf` is the bulk of a text font, so most of the size goes. It is not the
//! "few kilobytes" `I-058` asked for — that needs the renumbering above — and
//! the shortfall is stated rather than left to be measured.
//!
//! # What it refuses
//!
//! Anything it does not fully understand. A CFF font (`OTTO`), a collection,
//! a missing table: it hands the original back rather than guessing. A font
//! that is a little too large is a nuisance; a font that renders the wrong
//! glyph is a lie.

use std::collections::BTreeSet;

/// Keep the glyphs `text` needs and empty the rest.
///
/// Returns the original bytes unchanged whenever anything is unexpected —
/// there is no error case a caller could usefully act on, and refusing to
/// embed a font at all would be worse than embedding a whole one.
pub fn keep_glyphs_for(font: &[u8], text: &str) -> Vec<u8> {
    match cut(font, text) {
        Some(smaller) if smaller.len() < font.len() => smaller,
        _ => font.to_vec(),
    }
}

/// Unshaped horizontal advances from the embedded face, in drawing units.
/// Missing tables or glyphs leave the caller's existing estimate in place.
pub(crate) fn text_advance(font: &[u8], text: &str, size: f64) -> Option<f64> {
    if !size.is_finite()
        || size <= 0.0
        || !matches!(be32(font, 0)?, 0x0001_0000 | 0x7472_7565 | 0x4f54_544f)
    {
        return None;
    }
    let table = |tag: &[u8; 4]| {
        for i in 0..usize::from(be16(font, 4)?) {
            let at = 12 + i * 16;
            if font.get(at..at + 4)? == tag {
                let start = be32(font, at + 8)? as usize;
                let length = be32(font, at + 12)? as usize;
                return font.get(start..start.checked_add(length)?);
            }
        }
        None
    };
    let units = f64::from(be16(table(b"head")?, 18)?);
    let metrics = usize::from(be16(table(b"hhea")?, 34)?);
    let glyphs = usize::from(be16(table(b"maxp")?, 4)?);
    if units == 0.0 || metrics == 0 || metrics > glyphs {
        return None;
    }
    let advances = table(b"hmtx")?;
    advances.get(..metrics * 4)?;
    let cmap = table(b"cmap")?;
    let mut width = 0.0;
    for ch in text.chars() {
        let glyph = usize::from(glyph_of(cmap, 0, ch)?);
        if glyph == 0 || glyph >= glyphs {
            return None;
        }
        width += f64::from(be16(advances, glyph.min(metrics - 1) * 4)?);
    }
    Some(width * size / units)
}

/// A table's offset and length in the file.
struct Table {
    tag: [u8; 4],
    offset: usize,
    length: usize,
}

fn cut(font: &[u8], text: &str) -> Option<Vec<u8>> {
    // `0x00010000` or `true` — a TrueType outline font. `OTTO` is CFF, whose
    // outlines live in a table this does not read, and a font collection has
    // several fonts in it.
    let version = u32::from_be_bytes(font.get(..4)?.try_into().ok()?);
    if version != 0x0001_0000 && version != 0x7472_7565 {
        return None;
    }

    let count = be16(font, 4)? as usize;
    let mut tables = Vec::with_capacity(count);
    for i in 0..count {
        let at = 12 + i * 16;
        tables.push(Table {
            tag: font.get(at..at + 4)?.try_into().ok()?,
            offset: be32(font, at + 8)? as usize,
            length: be32(font, at + 12)? as usize,
        });
    }

    let find = |tag: &[u8; 4]| tables.iter().find(|t| t.tag == *tag);
    let head = find(b"head")?;
    let maxp = find(b"maxp")?;
    let loca = find(b"loca")?;
    let glyf = find(b"glyf")?;
    let cmap = find(b"cmap")?;

    let glyph_count = be16(font, maxp.offset + 4)? as usize;
    // 0 means the offsets are `u16` halved, 1 means `u32`. Anything else is a
    // font this does not understand.
    let long_loca = match be16(font, head.offset + 50)? {
        0 => false,
        1 => true,
        _ => return None,
    };

    let mut wanted: BTreeSet<u16> = BTreeSet::new();
    // Glyph 0 is `.notdef` and is drawn for anything the font lacks; dropping
    // it would replace a missing character with nothing at all.
    wanted.insert(0);
    for c in text.chars() {
        if let Some(id) = glyph_of(font, cmap.offset, c) {
            wanted.insert(id);
        }
    }
    // A composite glyph draws by referring to others. Emptying one of those
    // would leave an accent floating over nothing.
    let mut queue: Vec<u16> = wanted.iter().copied().collect();
    while let Some(id) = queue.pop() {
        for part in components(font, glyf, loca, long_loca, glyph_count, id) {
            if wanted.insert(part) {
                queue.push(part);
            }
        }
    }

    // The new `glyf`, and the offsets that index it.
    let mut outlines: Vec<u8> = Vec::with_capacity(glyf.length);
    let mut offsets: Vec<u32> = Vec::with_capacity(glyph_count + 1);
    for id in 0..glyph_count {
        offsets.push(outlines.len() as u32);
        if !wanted.contains(&(id as u16)) {
            // An empty glyph is two equal offsets and no bytes at all — the
            // format's own way of saying "nothing to draw", which is what a
            // space already is.
            continue;
        }
        let (from, to) = glyph_range(font, loca, long_loca, glyph_count, id as u16)?;
        outlines.extend_from_slice(font.get(glyf.offset + from..glyf.offset + to)?);
        // Every glyph starts on a word boundary in a well-formed font, and a
        // reader that assumes it will read garbage if this drifts.
        while !outlines.len().is_multiple_of(4) {
            outlines.push(0);
        }
    }
    offsets.push(outlines.len() as u32);

    let mut new_loca: Vec<u8> = Vec::with_capacity(offsets.len() * 4);
    for offset in &offsets {
        if long_loca {
            new_loca.extend_from_slice(&offset.to_be_bytes());
        } else {
            // The short form stores half the offset, so an odd one cannot be
            // written at all. Padding to four above keeps them even.
            new_loca.extend_from_slice(&((offset / 2) as u16).to_be_bytes());
        }
    }

    Some(rebuild(font, &tables, &new_loca, &outlines))
}

/// Write the file back out with `loca` and `glyf` replaced.
fn rebuild(font: &[u8], tables: &[Table], new_loca: &[u8], new_glyf: &[u8]) -> Vec<u8> {
    let count = tables.len();
    let directory = 12 + count * 16;
    let mut body: Vec<u8> = Vec::new();
    let mut entries: Vec<([u8; 4], u32, u32)> = Vec::with_capacity(count);

    for table in tables {
        let bytes: &[u8] = match &table.tag {
            b"loca" => new_loca,
            b"glyf" => new_glyf,
            _ => font
                .get(table.offset..table.offset + table.length)
                .unwrap_or(&[]),
        };
        let at = directory + body.len();
        entries.push((table.tag, at as u32, bytes.len() as u32));
        body.extend_from_slice(bytes);
        while !body.len().is_multiple_of(4) {
            body.push(0);
        }
    }

    let mut out = Vec::with_capacity(directory + body.len());
    out.extend_from_slice(&font[..12]);
    // Sorted by tag, which the format requires and a reader may rely on to
    // find a table by binary search.
    entries.sort_by_key(|(tag, _, _)| *tag);
    for (tag, offset, length) in entries {
        out.extend_from_slice(&tag);
        // The checksum is left at zero. Readers that verify one recompute it;
        // none of them refuse a font over it, and writing a wrong value would
        // be worse than writing none.
        out.extend_from_slice(&0u32.to_be_bytes());
        out.extend_from_slice(&offset.to_be_bytes());
        out.extend_from_slice(&length.to_be_bytes());
    }
    out.extend_from_slice(&body);
    out
}

/// The glyph ids a composite glyph is built from.
fn components(
    font: &[u8],
    glyf: &Table,
    loca: &Table,
    long_loca: bool,
    glyph_count: usize,
    id: u16,
) -> Vec<u16> {
    let Some((from, to)) = glyph_range(font, loca, long_loca, glyph_count, id) else {
        return Vec::new();
    };
    if to <= from + 10 {
        return Vec::new();
    }
    // A negative contour count means composite. Simple glyphs refer to
    // nothing.
    let Some(contours) = be16(font, glyf.offset + from) else {
        return Vec::new();
    };
    if (contours as i16) >= 0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut at = glyf.offset + from + 10;
    while let Some(flags) = be16(font, at) {
        let Some(index) = be16(font, at + 2) else {
            break;
        };
        out.push(index);

        // ARG_1_AND_2_ARE_WORDS
        at += 4 + if flags & 0x0001 != 0 { 4 } else { 2 };
        // WE_HAVE_A_SCALE / AND_AN_X_AND_Y_SCALE / A_TWO_BY_TWO
        at += if flags & 0x0008 != 0 {
            2
        } else if flags & 0x0040 != 0 {
            4
        } else if flags & 0x0080 != 0 {
            8
        } else {
            0
        };
        // MORE_COMPONENTS
        if flags & 0x0020 == 0 || at >= glyf.offset + to {
            break;
        }
    }
    out
}

fn glyph_range(
    font: &[u8],
    loca: &Table,
    long: bool,
    glyph_count: usize,
    id: u16,
) -> Option<(usize, usize)> {
    let i = id as usize;
    if i >= glyph_count {
        return None;
    }
    let read = |n: usize| -> Option<usize> {
        Some(if long {
            be32(font, loca.offset + n * 4)? as usize
        } else {
            be16(font, loca.offset + n * 2)? as usize * 2
        })
    };
    let from = read(i)?;
    let to = read(i + 1)?;
    if to < from { None } else { Some((from, to)) }
}

/// The glyph a character maps to, through the `cmap` subtable a diagram needs.
///
/// Format 4 and format 12 only — the two a modern font actually uses for
/// Unicode. A font offering neither is one this leaves alone.
pub(crate) fn glyph_of(font: &[u8], cmap: usize, c: char) -> Option<u16> {
    let tables = be16(font, cmap + 2)? as usize;
    let mut best: Option<usize> = None;
    for i in 0..tables {
        let at = cmap + 4 + i * 8;
        let platform = be16(font, at)?;
        let encoding = be16(font, at + 2)?;
        let offset = be32(font, at + 4)? as usize;
        // Windows Unicode BMP, Windows full Unicode, or a Unicode platform
        // table. Anything else is a legacy encoding a diagram will not want.
        let unicode = matches!((platform, encoding), (3, 1) | (3, 10) | (0, _));
        if unicode {
            best = Some(cmap + offset);
            if (platform, encoding) == (3, 10) {
                break;
            }
        }
    }
    let table = best?;

    match be16(font, table)? {
        4 => {
            let code = u32::from(c);
            if code > 0xFFFF {
                return None;
            }
            let code = code as u16;
            let segments = be16(font, table + 6)? as usize / 2;
            let ends = table + 14;
            let starts = ends + segments * 2 + 2;
            let deltas = starts + segments * 2;
            let ranges = deltas + segments * 2;

            for s in 0..segments {
                if be16(font, ends + s * 2)? < code {
                    continue;
                }
                if be16(font, starts + s * 2)? > code {
                    return None;
                }
                let delta = be16(font, deltas + s * 2)?;
                let range = be16(font, ranges + s * 2)?;
                return Some(if range == 0 {
                    code.wrapping_add(delta)
                } else {
                    let at = ranges
                        + s * 2
                        + range as usize
                        + (code - be16(font, starts + s * 2)?) as usize * 2;
                    let id = be16(font, at)?;
                    if id == 0 { 0 } else { id.wrapping_add(delta) }
                });
            }
            None
        }
        12 => {
            let code = u32::from(c);
            let groups = be32(font, table + 12)? as usize;
            for g in 0..groups {
                let at = table + 16 + g * 12;
                let start = be32(font, at)?;
                let end = be32(font, at + 4)?;
                if (start..=end).contains(&code) {
                    return u16::try_from(be32(font, at + 8)? + (code - start)).ok();
                }
            }
            None
        }
        _ => None,
    }
}

fn be16(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn be32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}
