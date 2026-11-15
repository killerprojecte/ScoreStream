//! Read a compressed MusicXML container without extracting archive paths.
//!
//! Only the manifest, optional MIME marker, and the exact first MusicXML root
//! are decompressed. The root is verified into an anonymous, seekable temporary
//! file: its size is not capped and its contents are never held in one buffer.
//! This is a container reader, not a claim of semantic MusicXML support. Pass
//! `MxlRoot::into_reader()` to the MusicXML scanner/adapter separately.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{self, BufReader, Read, Seek, Write},
};

use flate2::{Decompress, FlushDecompress, Status};
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use rawzip::{CompressionMethod, ReaderAt, ZipArchive, ZipArchiveEntryWayfinder};

use crate::{Format, MappingEffect, MappingReport};

const CONTAINER: &str = "META-INF/container.xml";
const XML_MIME: &str = "application/vnd.recordare.musicxml+xml";
const MXL_MIME: &[u8] = b"application/vnd.recordare.musicxml";
// All ZIP filename/extra/comment lengths are u16. This holds a complete record.
const HEADER_BUFFER: usize = 3 * u16::MAX as usize + 46;
const COPY_BUFFER: usize = 64 * 1024;

/// Limits on container metadata, not on musical events or total score bytes.
/// Applications may adjust these for containers with unusually many resources.
#[derive(Clone, Copy, Debug)]
pub struct ContainerLimits {
    pub max_entries: usize,
    pub max_manifest_bytes: usize,
    pub max_path_bytes: usize,
    pub max_total_path_bytes: usize,
}
impl Default for ContainerLimits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_manifest_bytes: 1024 * 1024,
            max_path_bytes: 4096,
            max_total_path_bytes: 16 * 1024 * 1024,
        }
    }
}

/// An integrity-checked root; dropping it closes/deletes the anonymous spool.
#[derive(Debug)]
pub struct MxlRoot {
    path: String,
    bytes: u64,
    file: File,
}
impl MxlRoot {
    pub fn root_path(&self) -> &str {
        &self.path
    }
    pub fn xml_bytes(&self) -> u64 {
        self.bytes
    }
    /// Starts at byte zero and supports the adapter's two-pass reader.
    pub fn into_reader(self) -> BufReader<File> {
        BufReader::new(self.file)
    }
}

/// Open a MusicXML ZIP container using the default metadata limits.
///
/// No URL, archive filename, link, DTD, or external entity is resolved. Root XML
/// bytes remain uninterpreted here; the strict XML scanner must validate them.
/// Runtime/disk use is proportional to the selected root. Disk/I/O failures
/// return a report and leave no persistent extracted file.
pub fn open<R: Read + Seek>(input: R) -> Result<MxlRoot, MappingReport> {
    open_with_limits(input, ContainerLimits::default())
}

pub fn open_with_limits<R: Read + Seek>(
    mut input: R,
    limits: ContainerLimits,
) -> Result<MxlRoot, MappingReport> {
    let input_bytes = input.seek(io::SeekFrom::End(0)).map_err(io_error)?;
    let mut buffer = vec![0; HEADER_BUFFER];
    let archive = ZipArchive::from_seekable(input, &mut buffer).map_err(zip_error)?;
    let directory_end = check_end_records(&archive)?;
    if archive.end_offset() != input_bytes {
        return Err(invalid("trailing bytes after ZIP end record"));
    }
    let mut central_end = archive.directory_offset();
    if archive.entries_hint() > limits.max_entries as u64 {
        return Err(limit("archive entries exceed container membership limit"));
    }
    let mut local_buffer = vec![0; 2 * u16::MAX as usize];
    let mut entries = archive.entries(&mut buffer);
    let mut members = BTreeMap::new();
    let mut spans = BTreeMap::new();
    let mut path_bytes = 0usize;
    let mut count = 0u64;
    while let Some(header) = entries.next_entry().map_err(zip_error)? {
        if header.central_directory_offset() != central_end {
            return Err(invalid("central directory record gap"));
        }
        central_end = central_end
            .checked_add(
                46 + header.file_path().as_ref().len() as u64
                    + header.extra_fields().remaining_bytes().len() as u64
                    + header.comment().as_bytes().len() as u64,
            )
            .ok_or_else(|| invalid("central directory size overflow"))?;
        count = count
            .checked_add(1)
            .ok_or_else(|| invalid("entry count overflow"))?;
        if count > archive.entries_hint() || count > limits.max_entries as u64 {
            return Err(invalid("central directory entry count mismatch"));
        }
        let raw_path = header.file_path();
        let name = std::str::from_utf8(raw_path.as_ref())
            .map_err(|_| invalid("entry path is not UTF-8"))?;
        validate_path(name, header.is_dir(), limits)?;
        path_bytes = path_bytes
            .checked_add(name.len())
            .ok_or_else(|| limit("path metadata overflow"))?;
        if path_bytes > limits.max_total_path_bytes {
            return Err(limit("archive path metadata exceeds membership limit"));
        }
        if members.contains_key(name) {
            return Err(invalid("duplicate archive entry path"));
        }
        let mode = header.mode().value() & 0o170000;
        if !matches!(mode, 0o100000 | 0o040000) || (mode == 0o040000) != header.is_dir() {
            return Err(invalid(
                "symlink, special file, or inconsistent directory entry",
            ));
        }
        // Only DEFLATE options, data descriptor, and UTF-8 flags are accepted.
        if header.flags().bits() & !0x080e != 0 {
            return Err(invalid("encrypted or unsupported ZIP entry flags"));
        }
        let method = header.compression_method();
        if !matches!(
            method,
            CompressionMethod::STORE | CompressionMethod::DEFLATE
        ) {
            return Err(invalid("only stored and DEFLATE ZIP entries are supported"));
        }
        let compressed = header.compressed_size_hint();
        let expanded = header.uncompressed_size_hint();
        if (method == CompressionMethod::STORE && compressed != expanded)
            || (method == CompressionMethod::DEFLATE
                && u128::from(expanded) > u128::from(compressed) * 1032)
            || (header.is_dir() && expanded != 0)
        {
            return Err(invalid("impossible ZIP entry size or compression ratio"));
        }

        let central = read_at::<46>(archive.get_ref(), header.central_directory_offset())?;
        let zip64 = check_extra_fields(header.extra_fields(), &central, false)?;
        let entry = archive.get_entry(header.wayfinder()).map_err(zip_error)?;
        let local = entry.local_header(&mut local_buffer).map_err(zip_error)?;
        let fixed_local = read_at::<30>(archive.get_ref(), header.local_header_offset())?;
        check_extra_fields(local.extra_fields(), &fixed_local, true)?;
        if local.file_path().as_ref() != name.as_bytes()
            || local.flags() != header.flags()
            || local.compression_method() != method
        {
            return Err(invalid("local and central ZIP headers disagree"));
        }
        if !header.flags().has_data_descriptor() {
            if local.crc32() != header.crc32()
                || local.compressed_size_hint() != compressed
                || local.uncompressed_size_hint() != expanded
            {
                return Err(invalid("local and central ZIP size/checksum disagree"));
            }
        } else if (local.crc32() != 0 && local.crc32() != header.crc32())
            || (local.compressed_size_hint() != 0 && local.compressed_size_hint() != compressed)
            || (local.uncompressed_size_hint() != 0 && local.uncompressed_size_hint() != expanded)
        {
            return Err(invalid("inconsistent data-descriptor local header"));
        }
        let (_, mut end) = entry.compressed_data_range();
        if let Some(descriptor) = entry.reader().data_descriptor().map_err(zip_error)? {
            if descriptor.crc32() != header.crc32()
                || descriptor.compressed_size() != compressed
                || descriptor.uncompressed_size() != expanded
            {
                return Err(invalid(
                    "ZIP data descriptor disagrees with central directory",
                ));
            }
            let signature = read_at::<4>(archive.get_ref(), end)?;
            let descriptor_len = if zip64 { 20 } else { 12 }
                + if signature == [0x50, 0x4b, 0x07, 0x08] {
                    4
                } else {
                    0
                };
            end = end
                .checked_add(descriptor_len)
                .ok_or_else(|| invalid("entry range overflow"))?;
        }
        let start = header.local_header_offset();
        if start >= end
            || end > archive.directory_offset()
            || spans
                .range(..=start)
                .next_back()
                .is_some_and(|(_, prior_end)| *prior_end > start)
            || spans
                .range(start..)
                .next()
                .is_some_and(|(next_start, _)| *next_start < end)
        {
            return Err(invalid("overlapping or out-of-bounds ZIP entry"));
        }
        spans.insert(start, end);
        if name == "mimetype"
            && (method != CompressionMethod::STORE
                || !local.extra_fields().remaining_bytes().is_empty()
                || expanded != MXL_MIME.len() as u64)
        {
            return Err(invalid("invalid MusicXML MIME marker"));
        }
        members.insert(
            name.to_owned(),
            Member {
                wayfinder: header.wayfinder(),
                method,
                crc: header.crc32(),
                directory: header.is_dir(),
            },
        );
    }
    if central_end != directory_end || spans.first_key_value().is_none_or(|(start, _)| *start != 0)
    {
        return Err(invalid(
            "invalid ZIP central directory extent or archive prefix",
        ));
    }
    if count != archive.entries_hint() {
        return Err(invalid("central directory entry count mismatch"));
    }
    let manifest = file_member(&members, CONTAINER)?;
    if manifest.wayfinder.uncompressed_size_hint() > limits.max_manifest_bytes as u64 {
        return Err(limit("container manifest exceeds metadata limit"));
    }
    let mut manifest_bytes = Vec::new();
    copy_member(&archive, manifest, &mut manifest_bytes)?;
    if let Some(marker) = members.get("mimetype") {
        let mut mime = Vec::new();
        copy_member(&archive, marker, &mut mime)?;
        if mime != MXL_MIME {
            return Err(invalid("invalid MusicXML MIME marker"));
        }
    }
    let path = parse_manifest(&manifest_bytes, &members, limits)?;
    let root = file_member(&members, &path)?;
    if path == CONTAINER || path == "mimetype" {
        return Err(invalid("container metadata cannot be the score root"));
    }
    let mut file = tempfile::tempfile().map_err(io_error)?;
    copy_member(&archive, root, &mut file)?;
    file.rewind().map_err(io_error)?;
    Ok(MxlRoot {
        path,
        bytes: root.wayfinder.uncompressed_size_hint(),
        file,
    })
}

#[derive(Clone, Copy)]
struct Member {
    wayfinder: ZipArchiveEntryWayfinder,
    method: CompressionMethod,
    crc: u32,
    directory: bool,
}
fn file_member<'a>(
    members: &'a BTreeMap<String, Member>,
    path: &str,
) -> Result<&'a Member, MappingReport> {
    members
        .get(path)
        .filter(|m| !m.directory)
        .ok_or_else(|| invalid("manifest references a missing file or directory"))
}
fn validate_path(
    path: &str,
    directory: bool,
    limits: ContainerLimits,
) -> Result<(), MappingReport> {
    if path.len() > limits.max_path_bytes {
        return Err(limit("archive path exceeds metadata limit"));
    }
    let components = if directory {
        path.strip_suffix('/').unwrap_or(path)
    } else {
        path
    };
    if components.is_empty()
        || path
            .chars()
            .any(|c| c.is_control() || c == '\\' || c == ':')
        || components
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
    {
        return Err(invalid(
            "archive paths must be exact relative UTF-8 paths without traversal or aliases",
        ));
    }
    Ok(())
}

// Validate extra-field framing and ZIP64 mandatory values rather than accepting
// the library's fallback hints when a ZIP64 field is missing/truncated.
fn check_extra_fields(
    mut fields: rawzip::extra_fields::ExtraFields<'_>,
    fixed: &[u8],
    local: bool,
) -> Result<bool, MappingReport> {
    let (compressed_at, expanded_at) = if local { (18, 22) } else { (20, 24) };
    let compressed64 = fixed[compressed_at..compressed_at + 4] == [255; 4];
    let expanded64 = fixed[expanded_at..expanded_at + 4] == [255; 4];
    let offset64 = !local && fixed[42..46] == [255; 4];
    let disk64 = !local && fixed[34..36] == [255; 2];
    if !local && !disk64 && fixed[34..36] != [0; 2] {
        return Err(invalid("split ZIP archives are unsupported"));
    }
    let mut found = false;
    for (id, value) in fields.by_ref() {
        if id.as_u16() != 1 {
            continue;
        }
        if found {
            return Err(invalid("duplicate ZIP64 extra field"));
        }
        found = true;
        let size_bytes =
            8 * (usize::from(expanded64) + usize::from(compressed64) + usize::from(offset64));
        if value.len() < size_bytes + if disk64 { 4 } else { 0 } {
            return Err(invalid("truncated ZIP64 extra field"));
        }
        if disk64 && value[size_bytes..size_bytes + 4] != [0; 4] {
            return Err(invalid("split ZIP archives are unsupported"));
        }
    }
    if !fields.remaining_bytes().is_empty()
        || (!found && (compressed64 || expanded64 || offset64 || disk64))
    {
        return Err(invalid("malformed or missing ZIP extra field"));
    }
    Ok(compressed64 || expanded64)
}

fn check_end_records<R: ReaderAt>(archive: &ZipArchive<R>) -> Result<u64, MappingReport> {
    let eocd = read_at::<22>(archive.get_ref(), archive.eocd_offset())?;
    if eocd[4..8] != [0; 4] || eocd[8..10] != eocd[10..12] {
        return Err(invalid("split ZIP archives are unsupported"));
    }
    let mut directory_size = u64::from(u32::from_le_bytes(eocd[12..16].try_into().unwrap()));
    let mut directory_offset = u64::from(u32::from_le_bytes(eocd[16..20].try_into().unwrap()));
    let mut directory_end = archive.eocd_offset();
    // ZIP64 records are mandatory when any traditional EOCD value overflows.
    if eocd[8..10] == [255; 2] || eocd[12..16] == [255; 4] || eocd[16..20] == [255; 4] {
        let locator_at = archive
            .eocd_offset()
            .checked_sub(20)
            .ok_or_else(|| invalid("missing ZIP64 locator"))?;
        let locator = read_at::<20>(archive.get_ref(), locator_at)?;
        if locator[..4] != [0x50, 0x4b, 0x06, 0x07]
            || locator[4..8] != [0; 4]
            || locator[16..20] != [1, 0, 0, 0]
        {
            return Err(invalid("invalid or split ZIP64 locator"));
        }
        let zip64_at = u64::from_le_bytes(locator[8..16].try_into().unwrap());
        let record = read_at::<56>(archive.get_ref(), zip64_at)?;
        if record[..4] != [0x50, 0x4b, 0x06, 0x06]
            || record[16..24] != [0; 8]
            || record[24..32] != record[32..40]
        {
            return Err(invalid("invalid or split ZIP64 end record"));
        }
        let record_size = u64::from_le_bytes(record[4..12].try_into().unwrap());
        if record_size < 44
            || zip64_at
                .checked_add(12)
                .and_then(|v| v.checked_add(record_size))
                != Some(locator_at)
        {
            return Err(invalid("invalid ZIP64 end record extent"));
        }
        directory_size = u64::from_le_bytes(record[40..48].try_into().unwrap());
        directory_offset = u64::from_le_bytes(record[48..56].try_into().unwrap());
        directory_end = zip64_at;
    }
    if directory_offset != archive.directory_offset()
        || directory_offset.checked_add(directory_size) != Some(directory_end)
    {
        return Err(invalid("inconsistent ZIP central directory extent"));
    }
    Ok(directory_end)
}
fn read_at<const N: usize>(reader: &impl ReaderAt, offset: u64) -> Result<[u8; N], MappingReport> {
    let mut buffer = [0u8; N];
    let mut read = 0;
    while read < N {
        let at = offset
            .checked_add(read as u64)
            .ok_or_else(|| invalid("ZIP offset overflow"))?;
        let n = reader.read_at(&mut buffer[read..], at).map_err(io_error)?;
        if n == 0 {
            return Err(invalid("truncated ZIP metadata"));
        }
        read += n;
    }
    Ok(buffer)
}

fn copy_member<R: ReaderAt>(
    archive: &ZipArchive<R>,
    member: &Member,
    output: &mut impl Write,
) -> Result<(), MappingReport> {
    let entry = archive.get_entry(member.wayfinder).map_err(zip_error)?;
    let mut input = entry.reader();
    let mut compressed = [0; COPY_BUFFER];
    let mut expanded = [0; COPY_BUFFER];
    let mut crc = rawzip::Crc32::new();
    let mut total = 0u64;
    let expected = member.wayfinder.uncompressed_size_hint();
    let mut write = |bytes: &[u8]| -> Result<(), MappingReport> {
        total = total
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| invalid("expanded size overflow"))?;
        if total > expected {
            return Err(invalid("ZIP entry expands beyond its declared size"));
        }
        crc.update(bytes);
        output.write_all(bytes).map_err(io_error)
    };
    if member.method == CompressionMethod::STORE {
        loop {
            let n = input.read(&mut compressed).map_err(io_error)?;
            if n == 0 {
                break;
            }
            write(&compressed[..n])?;
        }
    } else {
        let mut decoder = Decompress::new(false);
        let (mut pos, mut len) = (0, 0);
        loop {
            if pos == len {
                len = input.read(&mut compressed).map_err(io_error)?;
                pos = 0;
            }
            let before_in = decoder.total_in();
            let before_out = decoder.total_out();
            let status = decoder
                .decompress(
                    &compressed[pos..len],
                    &mut expanded,
                    if len == 0 {
                        FlushDecompress::Finish
                    } else {
                        FlushDecompress::None
                    },
                )
                .map_err(|_| invalid("invalid DEFLATE stream"))?;
            let used = (decoder.total_in() - before_in) as usize;
            let produced = (decoder.total_out() - before_out) as usize;
            pos += used;
            write(&expanded[..produced])?;
            if status == Status::StreamEnd {
                if decoder.total_in() != member.wayfinder.compressed_size_hint() {
                    return Err(invalid("DEFLATE stream has trailing or missing bytes"));
                }
                break;
            }
            if used == 0 && produced == 0 {
                return Err(invalid("truncated or stalled DEFLATE stream"));
            }
        }
    }
    if total != expected || crc.checksum() != member.crc {
        return Err(invalid("ZIP size or CRC integrity check failed"));
    }
    Ok(())
}

fn parse_manifest(
    bytes: &[u8],
    members: &BTreeMap<String, Member>,
    limits: ContainerLimits,
) -> Result<String, MappingReport> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| invalid("container manifest must be UTF-8"))?;
    if !text.chars().all(valid_xml_char) {
        return Err(invalid("invalid XML 1.0 character in container manifest"));
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_comments = true;
    let mut stack: Vec<&[u8]> = Vec::new();
    let (mut container, mut rootfiles, mut declaration) = (false, false, false);
    let mut roots = BTreeSet::new();
    let mut selected = None;
    loop {
        let event_start = reader.buffer_position();
        let event = reader
            .read_event()
            .map_err(|_| invalid("malformed container XML"))?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                validate_attribute_spacing(e.as_ref(), e.name().as_ref().len())?;
                let name: &[u8] = match (stack.as_slice(), e.name().as_ref()) {
                    ([], b"container") if !container => {
                        container = true;
                        b"container"
                    }
                    ([b"container"], b"rootfiles") if !rootfiles => {
                        rootfiles = true;
                        b"rootfiles"
                    }
                    ([b"container", b"rootfiles"], b"rootfile") => b"rootfile",
                    _ => return Err(invalid("unexpected container XML structure")),
                };
                if name == b"rootfile" {
                    let (path, media) = root_attributes(&e, &reader)?;
                    validate_path(&path, false, limits)?;
                    file_member(members, &path)?;
                    if roots.len() >= limits.max_entries {
                        return Err(limit("too many manifest rootfiles"));
                    }
                    if !roots.insert(path.clone()) {
                        return Err(invalid("duplicate manifest rootfile"));
                    }
                    if selected.is_none() {
                        if media.as_deref().is_some_and(|m| m != XML_MIME) {
                            return Err(invalid("first manifest rootfile must be MusicXML"));
                        }
                        selected = Some(path);
                    }
                } else if e.attributes().next().is_some() {
                    return Err(invalid("unsupported container XML attributes"));
                }
                if !empty {
                    stack.push(name);
                }
            }
            Event::End(e) => {
                if stack.pop() != Some(e.name().as_ref()) {
                    return Err(invalid("unbalanced container XML"));
                }
            }
            Event::Text(e) => {
                if !e.as_ref().iter().all(u8::is_ascii_whitespace) {
                    return Err(invalid("container elements may not contain text"));
                }
            }
            Event::Decl(e) => {
                if container || declaration || event_start != 0 {
                    return Err(invalid("misplaced container XML declaration"));
                }
                validate_declaration(e.as_ref())?;
                declaration = true;
            }
            Event::Comment(_) => {}
            Event::Eof => break,
            _ => {
                return Err(invalid(
                    "DTD, entities, processing instructions, and CDATA are not supported in container manifests",
                ));
            }
        }
    }
    if !stack.is_empty() || !container || !rootfiles {
        return Err(invalid("incomplete container XML"));
    }
    selected.ok_or_else(|| invalid("container has no MusicXML rootfile"))
}
fn root_attributes(
    e: &BytesStart<'_>,
    reader: &Reader<&[u8]>,
) -> Result<(String, Option<String>), MappingReport> {
    let (mut path, mut media) = (None, None);
    for attribute in e.attributes() {
        let attr = attribute.map_err(|_| invalid("malformed or duplicate container attribute"))?;
        if attr.value.contains(&b'<') {
            return Err(invalid("unescaped less-than in container attribute"));
        }
        let value = attr
            .decode_and_unescape_value(reader.decoder())
            .map_err(|_| invalid("invalid container attribute/entity"))?
            .into_owned();
        if !value.chars().all(valid_xml_char) {
            return Err(invalid("invalid decoded XML 1.0 attribute character"));
        }
        match attr.key.as_ref() {
            b"full-path" => path = Some(value),
            b"media-type" => media = Some(value),
            _ => return Err(invalid("unsupported rootfile attribute")),
        }
    }
    Ok((
        path.ok_or_else(|| invalid("rootfile has no full-path"))?,
        media,
    ))
}

fn valid_xml_char(c: char) -> bool {
    matches!(c, '\u{9}' | '\u{a}' | '\u{d}' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}
fn xml_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}
// quick-xml permits adjacent quoted attributes. XML requires separating S.
// Boundaries are checked on raw bytes; unescaping happens only afterward.
fn validate_attribute_spacing(bytes: &[u8], name_len: usize) -> Result<(), MappingReport> {
    let mut pos = name_len;
    while pos < bytes.len() {
        if !xml_space(bytes[pos]) {
            return Err(invalid("XML attributes require whitespace separators"));
        }
        while pos < bytes.len() && xml_space(bytes[pos]) {
            pos += 1;
        }
        if pos == bytes.len() {
            break;
        }
        let name_start = pos;
        while pos < bytes.len() && !xml_space(bytes[pos]) && bytes[pos] != b'=' {
            pos += 1;
        }
        if pos == name_start {
            return Err(invalid("empty XML attribute name"));
        }
        while pos < bytes.len() && xml_space(bytes[pos]) {
            pos += 1;
        }
        if bytes.get(pos) != Some(&b'=') {
            return Err(invalid("XML attribute requires equals"));
        }
        pos += 1;
        while pos < bytes.len() && xml_space(bytes[pos]) {
            pos += 1;
        }
        let quote = *bytes
            .get(pos)
            .ok_or_else(|| invalid("missing XML attribute value"))?;
        if !matches!(quote, b'\'' | b'"') {
            return Err(invalid("XML attributes must be quoted"));
        }
        pos += 1;
        while pos < bytes.len() && bytes[pos] != quote {
            pos += 1;
        }
        if pos == bytes.len() {
            return Err(invalid("unterminated XML attribute value"));
        }
        pos += 1;
    }
    Ok(())
}

fn validate_declaration(bytes: &[u8]) -> Result<(), MappingReport> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid("invalid XML declaration"))?;
    validate_attribute_spacing(bytes, 3)?;
    let start = BytesStart::from_content(text, 3);
    let mut rank = 0;
    for attribute in start.attributes() {
        let attribute =
            attribute.map_err(|_| invalid("malformed or duplicate XML declaration attribute"))?;
        let next = match attribute.key.as_ref() {
            b"version" if attribute.value.as_ref() == b"1.0" => 1,
            b"encoding" if attribute.value.eq_ignore_ascii_case(b"UTF-8") => 2,
            b"standalone" if matches!(attribute.value.as_ref(), b"yes" | b"no") => 3,
            _ => return Err(invalid("unsupported XML declaration attribute")),
        };
        if next <= rank || (rank == 0 && next != 1) {
            return Err(invalid("invalid XML declaration order"));
        }
        rank = next;
    }
    if rank == 0 {
        return Err(invalid("XML declaration requires a version"));
    }
    Ok(())
}

fn report(effect: MappingEffect, message: impl Into<String>) -> MappingReport {
    let mut report = MappingReport::default();
    report.push(Format::MusicXml, effect, "mxl_container", "/", message);
    report
}
fn invalid(message: impl Into<String>) -> MappingReport {
    report(MappingEffect::Invalid, message)
}
fn limit(message: impl Into<String>) -> MappingReport {
    report(MappingEffect::ResourceLimit, message)
}
fn zip_error(error: rawzip::Error) -> MappingReport {
    invalid(format!("invalid ZIP container: {error}"))
}
fn io_error(error: io::Error) -> MappingReport {
    invalid(format!("container I/O failed: {error}"))
}
