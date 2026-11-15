use crate::{Architecture, CatalogError, Result};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryFormat {
    Elf,
    Pe,
    MachO,
    MachOUniversal,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HeaderInfo {
    pub format: BinaryFormat,
    pub architectures: Vec<Architecture>,
    pub bits: Option<u8>,
    pub byte_order: String,
    /// Always false: imports, signatures, ABI and exported factories are unchecked.
    pub loadability_verified: bool,
}
fn invalid(message: &str) -> CatalogError {
    CatalogError::Invalid(message.into())
}
fn range(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8]> {
    bytes
        .get(
            offset
                ..offset
                    .checked_add(length)
                    .ok_or_else(|| invalid("Header offset overflow"))?,
        )
        .ok_or_else(|| invalid("Truncated header or header outside inspection budget"))
}
fn u16_at(bytes: &[u8], offset: usize, little: bool) -> Result<u16> {
    let value = range(bytes, offset, 2)?.try_into().expect("checked length");
    Ok(if little {
        u16::from_le_bytes(value)
    } else {
        u16::from_be_bytes(value)
    })
}
fn u32_at(bytes: &[u8], offset: usize, little: bool) -> Result<u32> {
    let value = range(bytes, offset, 4)?.try_into().expect("checked length");
    Ok(if little {
        u32::from_le_bytes(value)
    } else {
        u32::from_be_bytes(value)
    })
}
fn u64_at(bytes: &[u8], offset: usize, little: bool) -> Result<u64> {
    let value = range(bytes, offset, 8)?.try_into().expect("checked length");
    Ok(if little {
        u64::from_le_bytes(value)
    } else {
        u64::from_be_bytes(value)
    })
}
fn info(
    format: BinaryFormat,
    architectures: Vec<Architecture>,
    bits: Option<u8>,
    little: bool,
) -> HeaderInfo {
    HeaderInfo {
        format,
        architectures,
        bits,
        byte_order: if little { "little" } else { "big" }.into(),
        loadability_verified: false,
    }
}
fn mach_arch(cpu: u32) -> Architecture {
    match cpu {
        7 => Architecture::X86,
        0x01000007 => Architecture::X86_64,
        12 => Architecture::Arm,
        0x0100000c => Architecture::Arm64,
        n => Architecture::Unknown(n),
    }
}

/// Pure bounded header interpretation. The caller supplies at most the first
/// 64 KiB and the observed file size. Never validates executable code or loads it.
pub fn inspect_header(bytes: &[u8], file_size: u64) -> Result<HeaderInfo> {
    if bytes.len() > 65536 || bytes.len() as u64 > file_size {
        return Err(CatalogError::Limit(
            "Invalid header inspection extent".into(),
        ));
    }
    let magic = range(bytes, 0, 4)?;
    if magic == b"\x7fELF" {
        range(bytes, 0, 16)?;
        let bits = match *range(bytes, 4, 1)?.first().expect("checked") {
            1 => 32,
            2 => 64,
            _ => return Err(invalid("Unknown ELF class")),
        };
        let little = match bytes[5] {
            1 => true,
            2 => false,
            _ => return Err(invalid("Unknown ELF byte order")),
        };
        range(bytes, 0, if bits == 64 { 64 } else { 52 })?;
        if bytes[6] != 1 || u32_at(bytes, 20, little)? != 1 || u16_at(bytes, 16, little)? != 3 {
            return Err(invalid("Not a current ELF shared-object header"));
        }
        let architecture = match u16_at(bytes, 18, little)? {
            3 => Architecture::X86,
            62 => Architecture::X86_64,
            40 => Architecture::Arm,
            183 => Architecture::Arm64,
            n => Architecture::Unknown(u32::from(n)),
        };
        return Ok(info(
            BinaryFormat::Elf,
            vec![architecture],
            Some(bits),
            little,
        ));
    }
    if magic[..2] == *b"MZ" {
        let offset = u32_at(bytes, 0x3c, true)? as usize;
        if offset < 64 || range(bytes, offset, 4)? != b"PE\0\0" {
            return Err(invalid("Bad PE signature/offset"));
        }
        range(bytes, offset, 26)?;
        if u16_at(bytes, offset + 22, true)? & 0x2000 == 0 {
            return Err(invalid("PE image is not marked DLL"));
        }
        let optional_bytes = usize::from(u16_at(bytes, offset + 20, true)?);
        if optional_bytes < 2 {
            return Err(invalid("Missing PE optional header"));
        }
        range(bytes, offset + 24, optional_bytes)?;
        let bits = match u16_at(bytes, offset + 24, true)? {
            0x10b => 32,
            0x20b => 64,
            _ => return Err(invalid("Unknown PE optional-header type")),
        };
        let architecture = match u16_at(bytes, offset + 4, true)? {
            0x014c => Architecture::X86,
            0x8664 => Architecture::X86_64,
            0x01c0 | 0x01c4 => Architecture::Arm,
            0xaa64 => Architecture::Arm64,
            0xa641 => Architecture::Arm64Ec,
            0xa64e => Architecture::Arm64X,
            n => Architecture::Unknown(u32::from(n)),
        };
        return Ok(info(BinaryFormat::Pe, vec![architecture], Some(bits), true));
    }
    let word = u32_at(bytes, 0, false)?;
    if matches!(word, 0xfeedface | 0xcefaedfe | 0xfeedfacf | 0xcffaedfe) {
        let little = matches!(word, 0xcefaedfe | 0xcffaedfe);
        let bits = if matches!(word, 0xfeedfacf | 0xcffaedfe) {
            64
        } else {
            32
        };
        range(bytes, 0, if bits == 64 { 32 } else { 28 })?;
        if !matches!(u32_at(bytes, 12, little)?, 6 | 8) {
            return Err(invalid("Mach-O image is not a dylib/bundle"));
        }
        return Ok(info(
            BinaryFormat::MachO,
            vec![mach_arch(u32_at(bytes, 4, little)?)],
            Some(bits),
            little,
        ));
    }
    if matches!(word, 0xcafebabe | 0xbebafeca | 0xcafebabf | 0xbfbafeca) {
        let little = matches!(word, 0xbebafeca | 0xbfbafeca);
        let wide = matches!(word, 0xcafebabf | 0xbfbafeca);
        let count = u32_at(bytes, 4, little)? as usize;
        if count == 0 || count > 64 {
            return Err(CatalogError::Limit(
                "Universal architecture count outside 1..64".into(),
            ));
        }
        let stride = if wide { 32 } else { 20 };
        let table_end = 8 + count * stride;
        range(bytes, 0, table_end)?;
        let mut architectures = Vec::new();
        let mut extents = Vec::new();
        for index in 0..count {
            let at = 8 + index * stride;
            let architecture = mach_arch(u32_at(bytes, at, little)?);
            let (start, size) = if wide {
                (
                    u64_at(bytes, at + 8, little)?,
                    u64_at(bytes, at + 16, little)?,
                )
            } else {
                (
                    u64::from(u32_at(bytes, at + 8, little)?),
                    u64::from(u32_at(bytes, at + 12, little)?),
                )
            };
            let end = start
                .checked_add(size)
                .filter(|end| *end <= file_size)
                .ok_or_else(|| invalid("Universal slice exceeds file size"))?;
            if start < table_end as u64 || size == 0 {
                return Err(invalid("Universal slice overlaps table or is empty"));
            }
            extents.push((start, end));
            if !architectures.contains(&architecture) {
                architectures.push(architecture);
            }
        }
        extents.sort_unstable();
        if extents.windows(2).any(|pair| pair[0].1 > pair[1].0) {
            return Err(invalid("Overlapping universal slices"));
        }
        return Ok(info(
            BinaryFormat::MachOUniversal,
            architectures,
            None,
            little,
        ));
    }
    Err(invalid("Unrecognized executable header"))
}
