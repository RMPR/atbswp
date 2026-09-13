//! Just enough zip to append and read *stored* entries at the end of an
//! executable.  An APE is a valid zip archive, so the player finds these
//! entries through cosmopolitan's `/zip/` filesystem; ordinary `unzip`
//! lists them too.  No compression, no zip64 (macros are small).

use crate::{Error, Result};

const LOCAL_SIG: u32 = 0x0403_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const EOCD_SIG: u32 = 0x0605_4b50;

fn rd16(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn rd32(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}

/// A central-directory entry we know how to handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub method: u16,
    pub crc: u32,
    pub size: u32,
    /// Offset of the local file header.
    pub header_off: u32,
    /// Offset of the data (stored entries only).
    pub data_off: usize,
    /// Verbatim central directory record, re-emitted when appending.
    raw: Vec<u8>,
}

/// Parsed archive layout: entries plus where the central directory starts
/// (everything from there on is rewritten when appending).
pub struct Archive {
    pub entries: Vec<Entry>,
    pub cd_off: usize,
}

/// Parse the zip that ends `buf`, if any.
pub fn parse(buf: &[u8]) -> Option<Archive> {
    if buf.len() < 22 {
        return None;
    }
    let stop = buf.len().saturating_sub(22 + 65535);
    let eocd = (stop..=buf.len() - 22)
        .rev()
        .find(|&i| rd32(buf, i) == EOCD_SIG)?;
    let count = rd16(buf, eocd + 10) as usize;
    let cd_size = rd32(buf, eocd + 12) as usize;
    let cd_off = rd32(buf, eocd + 16) as usize;
    if cd_off + cd_size > eocd {
        return None;
    }
    let mut entries = Vec::with_capacity(count);
    let mut p = cd_off;
    for _ in 0..count {
        if p + 46 > eocd || rd32(buf, p) != CENTRAL_SIG {
            return None;
        }
        let nlen = rd16(buf, p + 28) as usize;
        let xlen = rd16(buf, p + 30) as usize;
        let clen = rd16(buf, p + 32) as usize;
        let end = p + 46 + nlen + xlen + clen;
        if end > eocd {
            return None;
        }
        let header_off = rd32(buf, p + 42);
        let h = header_off as usize;
        if h + 30 > buf.len() || rd32(buf, h) != LOCAL_SIG {
            return None;
        }
        let data_off = h + 30 + rd16(buf, h + 26) as usize + rd16(buf, h + 28) as usize;
        entries.push(Entry {
            name: String::from_utf8_lossy(&buf[p + 46..p + 46 + nlen]).into_owned(),
            method: rd16(buf, p + 10),
            crc: rd32(buf, p + 16),
            size: rd32(buf, p + 24),
            header_off,
            data_off,
            raw: buf[p..end].to_vec(),
        });
        p = end;
    }
    Some(Archive { entries, cd_off })
}

/// Bytes of a stored entry.
pub fn read<'a>(buf: &'a [u8], name: &str) -> Result<Option<&'a [u8]>> {
    let Some(a) = parse(buf) else { return Ok(None) };
    let Some(e) = a.entries.iter().find(|e| e.name == name) else {
        return Ok(None);
    };
    if e.method != 0 {
        return Err(Error::Invalid(format!("zip entry {name} is compressed")));
    }
    let end = e.data_off + e.size as usize;
    if end > buf.len() {
        return Err(Error::Invalid(format!("zip entry {name} is truncated")));
    }
    Ok(Some(&buf[e.data_off..end]))
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in data {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// Return `buf` with `name` stored as the **last** entry.  A previous entry
/// of that name is dropped, which only works cleanly if it was last too
/// (the case for `macro.bin`, which `atbswp export` always appends last);
/// any other duplicate is rejected.
pub fn append_stored(buf: &[u8], name: &str, data: &[u8]) -> Result<Vec<u8>> {
    let (mut out, mut records): (Vec<u8>, Vec<Vec<u8>>) = match parse(buf) {
        Some(a) => {
            let mut entries = a.entries;
            let mut prefix_end = a.cd_off;
            if let Some(pos) = entries.iter().position(|e| e.name == name) {
                if pos != entries.len() - 1 {
                    return Err(Error::Invalid(format!(
                        "zip entry {name} exists and is not last"
                    )));
                }
                prefix_end = entries[pos].header_off as usize;
                entries.pop();
            }
            (
                buf[..prefix_end].to_vec(),
                entries.into_iter().map(|e| e.raw).collect(),
            )
        }
        None => (buf.to_vec(), vec![]),
    };
    if out.len() > u32::MAX as usize - data.len() - 1024 {
        return Err(Error::Invalid("file too large for zip32".into()));
    }
    let crc = crc32(data);
    let size = data.len() as u32;
    let header_off = out.len() as u32;
    let name_b = name.as_bytes();

    // local file header
    out.extend_from_slice(&LOCAL_SIG.to_le_bytes());
    out.extend_from_slice(&20u16.to_le_bytes()); // version needed
    out.extend_from_slice(&0u16.to_le_bytes()); // flags
    out.extend_from_slice(&0u16.to_le_bytes()); // method: stored
    out.extend_from_slice(&0u16.to_le_bytes()); // mtime
    out.extend_from_slice(&0x21u16.to_le_bytes()); // mdate: 1980-01-01
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&(name_b.len() as u16).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // extra
    out.extend_from_slice(name_b);
    out.extend_from_slice(data);

    // central directory record for the new entry
    let mut rec = Vec::with_capacity(46 + name_b.len());
    rec.extend_from_slice(&CENTRAL_SIG.to_le_bytes());
    rec.extend_from_slice(&0x0314u16.to_le_bytes()); // made by: unix, 2.0
    rec.extend_from_slice(&20u16.to_le_bytes());
    rec.extend_from_slice(&0u16.to_le_bytes());
    rec.extend_from_slice(&0u16.to_le_bytes());
    rec.extend_from_slice(&0u16.to_le_bytes());
    rec.extend_from_slice(&0x21u16.to_le_bytes());
    rec.extend_from_slice(&crc.to_le_bytes());
    rec.extend_from_slice(&size.to_le_bytes());
    rec.extend_from_slice(&size.to_le_bytes());
    rec.extend_from_slice(&(name_b.len() as u16).to_le_bytes());
    rec.extend_from_slice(&0u16.to_le_bytes()); // extra
    rec.extend_from_slice(&0u16.to_le_bytes()); // comment
    rec.extend_from_slice(&0u16.to_le_bytes()); // disk
    rec.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
    rec.extend_from_slice(&(0o100644u32 << 16).to_le_bytes()); // external attrs
    rec.extend_from_slice(&header_off.to_le_bytes());
    rec.extend_from_slice(name_b);
    records.push(rec);

    let cd_off = out.len() as u32;
    for r in &records {
        out.extend_from_slice(r);
    }
    let cd_size = out.len() as u32 - cd_off;
    write_eocd(&mut out, records.len() as u16, cd_size, cd_off);
    Ok(out)
}

/// Return `buf` without its last entry, which must be `name`.  When it was
/// the only entry the whole zip section disappears.
pub fn remove_last(buf: &[u8], name: &str) -> Result<Vec<u8>> {
    let a = parse(buf).ok_or_else(|| Error::Invalid("no zip section".into()))?;
    let Some(last) = a.entries.last() else {
        return Err(Error::Invalid("empty zip".into()));
    };
    if last.name != name {
        return Err(Error::Invalid(format!(
            "zip entry {name} is not the last entry"
        )));
    }
    let mut out = buf[..last.header_off as usize].to_vec();
    let records: Vec<&Vec<u8>> = a.entries[..a.entries.len() - 1]
        .iter()
        .map(|e| &e.raw)
        .collect();
    if records.is_empty() {
        return Ok(out);
    }
    let cd_off = out.len() as u32;
    for r in &records {
        out.extend_from_slice(r);
    }
    let cd_size = out.len() as u32 - cd_off;
    write_eocd(&mut out, records.len() as u16, cd_size, cd_off);
    Ok(out)
}

fn write_eocd(out: &mut Vec<u8>, count: u16, cd_size: u32, cd_off: u32) {
    out.extend_from_slice(&EOCD_SIG.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // disk
    out.extend_from_slice(&0u16.to_le_bytes()); // cd disk
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_off.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_known_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn append_to_plain_file_then_replace() {
        let exe = b"MZqFpD='\nnot really a program".to_vec();
        let a = append_stored(&exe, "helper", b"HELPER").unwrap();
        assert!(a.starts_with(&exe));
        assert_eq!(read(&a, "helper").unwrap(), Some(&b"HELPER"[..]));
        let b = append_stored(&a, "macro.bin", b"v1").unwrap();
        assert_eq!(read(&b, "helper").unwrap(), Some(&b"HELPER"[..]));
        assert_eq!(read(&b, "macro.bin").unwrap(), Some(&b"v1"[..]));
        // replacing the last entry leaves no dead bytes behind
        let c = append_stored(&b, "macro.bin", b"version two").unwrap();
        assert_eq!(read(&c, "macro.bin").unwrap(), Some(&b"version two"[..]));
        assert_eq!(parse(&c).unwrap().entries.len(), 2);
        assert_eq!(c.len(), b.len() + "version two".len() - "v1".len());
        // replacing a non-last entry is refused
        assert!(append_stored(&c, "helper", b"x").is_err());
        assert_eq!(read(&exe, "macro.bin").unwrap(), None);
        // removing the last entry restores the previous archive exactly
        assert_eq!(remove_last(&c, "macro.bin").unwrap(), a);
        assert_eq!(remove_last(&a, "helper").unwrap(), exe);
        assert!(remove_last(&c, "helper").is_err());
    }
}
