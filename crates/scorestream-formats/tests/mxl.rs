use scorestream_formats::mxl::{self, ContainerLimits};
use std::io::{Cursor, Read, Seek, Write};

const SCORE: &[u8] =
    b"<?xml version=\"1.0\"?><score-partwise version=\"4.0\"><part-list/></score-partwise>";
const CONTAINER: &str = "META-INF/container.xml";
fn manifest(roots: &str) -> Vec<u8> {
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><container><rootfiles>{roots}</rootfiles></container>").into_bytes()
}
fn simple_manifest(path: &str) -> Vec<u8> {
    manifest(&format!("<rootfile full-path=\"{path}\"/>"))
}

#[derive(Clone)]
struct Entry<'a> {
    name: &'a str,
    bytes: &'a [u8],
    method: u16,
    flags: u16,
    mode: u32,
    zip64: bool,
    descriptor: bool,
}
impl<'a> Entry<'a> {
    fn new(name: &'a str, bytes: &'a [u8]) -> Self {
        Self {
            name,
            bytes,
            method: 8,
            flags: 2048,
            mode: 0o100644,
            zip64: false,
            descriptor: false,
        }
    }
    fn stored(mut self) -> Self {
        self.method = 0;
        self
    }
}
fn put16(v: &mut Vec<u8>, x: u16) {
    v.extend(x.to_le_bytes());
}
fn put32(v: &mut Vec<u8>, x: u32) {
    v.extend(x.to_le_bytes());
}
fn put64(v: &mut Vec<u8>, x: u64) {
    v.extend(x.to_le_bytes());
}
// Independent, intentionally small ZIP fixture generator. Mutations below cover
// metadata contradictions that a production ZIP writer normally cannot emit.
fn zip(entries: &[Entry<'_>]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut central = Vec::new();
    for e in entries {
        let offset = result.len() as u64;
        let compressed = if e.method == 8 {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(e.bytes).unwrap();
            encoder.finish().unwrap()
        } else {
            e.bytes.to_vec()
        };
        let mut crc = rawzip::Crc32::new();
        crc.update(e.bytes);
        let crc = crc.checksum();
        let flags = e.flags | if e.descriptor { 8 } else { 0 };
        let mut extra = Vec::new();
        if e.zip64 {
            put16(&mut extra, 1);
            put16(&mut extra, 16);
            put64(&mut extra, e.bytes.len() as u64);
            put64(&mut extra, compressed.len() as u64);
        }
        put32(&mut result, 0x04034b50);
        put16(&mut result, if e.zip64 { 45 } else { 20 });
        put16(&mut result, flags);
        put16(&mut result, e.method);
        put32(&mut result, 0);
        put32(&mut result, if e.descriptor { 0 } else { crc });
        put32(
            &mut result,
            if e.zip64 {
                u32::MAX
            } else if e.descriptor {
                0
            } else {
                compressed.len() as u32
            },
        );
        put32(
            &mut result,
            if e.zip64 {
                u32::MAX
            } else if e.descriptor {
                0
            } else {
                e.bytes.len() as u32
            },
        );
        put16(&mut result, e.name.len() as u16);
        put16(&mut result, extra.len() as u16);
        result.extend(e.name.as_bytes());
        result.extend(&extra);
        result.extend(&compressed);
        if e.descriptor {
            put32(&mut result, 0x08074b50);
            put32(&mut result, crc);
            if e.zip64 {
                put64(&mut result, compressed.len() as u64);
                put64(&mut result, e.bytes.len() as u64);
            } else {
                put32(&mut result, compressed.len() as u32);
                put32(&mut result, e.bytes.len() as u32);
            }
        }
        if e.zip64 {
            extra[2..4].copy_from_slice(&24u16.to_le_bytes());
            put64(&mut extra, offset);
        }
        put32(&mut central, 0x02014b50);
        put16(&mut central, 0x031e);
        put16(&mut central, if e.zip64 { 45 } else { 20 });
        put16(&mut central, flags);
        put16(&mut central, e.method);
        put32(&mut central, 0);
        put32(&mut central, crc);
        put32(
            &mut central,
            if e.zip64 {
                u32::MAX
            } else {
                compressed.len() as u32
            },
        );
        put32(
            &mut central,
            if e.zip64 {
                u32::MAX
            } else {
                e.bytes.len() as u32
            },
        );
        put16(&mut central, e.name.len() as u16);
        put16(&mut central, extra.len() as u16);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put32(&mut central, e.mode << 16);
        put32(&mut central, if e.zip64 { u32::MAX } else { offset as u32 });
        central.extend(e.name.as_bytes());
        central.extend(&extra);
    }
    let offset = result.len() as u64;
    let size = central.len() as u64;
    result.extend(central);
    let zip64 = entries.iter().any(|e| e.zip64);
    if zip64 {
        let zip64_at = result.len() as u64;
        put32(&mut result, 0x06064b50);
        put64(&mut result, 44);
        put16(&mut result, 45);
        put16(&mut result, 45);
        put32(&mut result, 0);
        put32(&mut result, 0);
        put64(&mut result, entries.len() as u64);
        put64(&mut result, entries.len() as u64);
        put64(&mut result, size);
        put64(&mut result, offset);
        put32(&mut result, 0x07064b50);
        put32(&mut result, 0);
        put64(&mut result, zip64_at);
        put32(&mut result, 1);
    }
    put32(&mut result, 0x06054b50);
    put16(&mut result, 0);
    put16(&mut result, 0);
    put16(
        &mut result,
        if zip64 {
            u16::MAX
        } else {
            entries.len() as u16
        },
    );
    put16(
        &mut result,
        if zip64 {
            u16::MAX
        } else {
            entries.len() as u16
        },
    );
    put32(&mut result, if zip64 { u32::MAX } else { size as u32 });
    put32(&mut result, if zip64 { u32::MAX } else { offset as u32 });
    put16(&mut result, 0);
    result
}
fn archive(manifest: &[u8]) -> Vec<u8> {
    zip(&[
        Entry::new(CONTAINER, manifest),
        Entry::new("score.xml", SCORE),
    ])
}
fn error(bytes: Vec<u8>) -> String {
    mxl::open(Cursor::new(bytes)).unwrap_err().diagnostics[0]
        .message
        .clone()
}
fn assert_error(bytes: Vec<u8>, fragment: &str) {
    let actual = error(bytes);
    assert!(
        actual.contains(fragment),
        "expected {fragment:?}, got {actual:?}"
    );
}
fn headers(bytes: &[u8]) -> Vec<usize> {
    bytes
        .windows(4)
        .enumerate()
        .filter_map(|(n, w)| (w == b"PK\x01\x02").then_some(n))
        .collect()
}

#[test]
fn reads_stored_deflated_and_zip64_roots_without_changing_bytes() {
    for stored in [false, true] {
        for zip64 in [false, true] {
            for descriptor in [false, true] {
                let manifest = simple_manifest("scores/交響曲.musicxml");
                let mut root = Entry::new("scores/交響曲.musicxml", SCORE);
                if stored {
                    root = root.stored();
                }
                root.zip64 = zip64;
                root.descriptor = descriptor;
                let archive = zip(&[Entry::new(CONTAINER, &manifest), root]);
                let root = mxl::open(Cursor::new(archive)).unwrap();
                assert_eq!(root.root_path(), "scores/交響曲.musicxml");
                assert_eq!(root.xml_bytes(), SCORE.len() as u64);
                let mut reader = root.into_reader();
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes).unwrap();
                assert_eq!(bytes, SCORE);
                reader.rewind().unwrap();
                assert_eq!(reader.read(&mut [0; 1]).unwrap(), 1);
            }
        }
    }
}
#[test]
fn manifest_order_not_archive_order_selects_primary_score() {
    let manifest = manifest(
        "<rootfile full-path=\"main.xml\" media-type=\"application/vnd.recordare.musicxml+xml\"/><rootfile full-path=\"part.xml\"/><rootfile full-path=\"print.pdf\" media-type=\"application/pdf\"/>",
    );
    let bytes = zip(&[
        Entry::new("part.xml", b"not primary"),
        Entry::new("print.pdf", b"pdf"),
        Entry::new(CONTAINER, &manifest),
        Entry::new("main.xml", SCORE),
    ]);
    assert_eq!(
        mxl::open(Cursor::new(bytes)).unwrap().root_path(),
        "main.xml"
    );
}
#[test]
fn supports_old_manifests_without_mimetype_or_media_type() {
    let manifest = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container>\n<rootfiles>\n<rootfile full-path=\"score.xml\">\n</rootfile>\n</rootfiles>\n</container>";
    assert_eq!(
        mxl::open(Cursor::new(archive(manifest)))
            .unwrap()
            .root_path(),
        "score.xml"
    );
}
#[test]
fn no_fallback_to_an_unlisted_or_later_root() {
    assert_error(zip(&[Entry::new("score.xml", SCORE)]), "missing file");
    assert_error(
        archive(&manifest(
            "<rootfile full-path=\"score.xml\" media-type=\"application/pdf\"/><rootfile full-path=\"other.xml\"/>",
        )),
        "first manifest rootfile",
    );
    assert_error(
        archive(&manifest(
            "<rootfile full-path=\"missing.xml\"/><rootfile full-path=\"score.xml\"/>",
        )),
        "missing file",
    );
}
#[test]
fn rejects_duplicate_and_unsafe_paths_in_any_member() {
    let manifest = simple_manifest("score.xml");
    assert_error(
        zip(&[
            Entry::new(CONTAINER, &manifest),
            Entry::new("score.xml", SCORE),
            Entry::new("score.xml", SCORE),
        ]),
        "duplicate archive",
    );
    for path in [
        "../score.xml",
        "/score.xml",
        "./score.xml",
        "x/../score.xml",
        "x//score.xml",
        "C:/score.xml",
        "x\\score.xml",
        "https://example.test/score.xml",
        "score\0.xml",
        "",
    ] {
        assert_error(
            zip(&[
                Entry::new(CONTAINER, &manifest),
                Entry::new("score.xml", SCORE),
                Entry::new(path, b"x"),
            ]),
            "paths must be exact",
        );
        assert!(mxl::open(Cursor::new(archive(&simple_manifest(path)))).is_err());
    }
}
#[test]
fn rejects_symlinks_special_entries_encryption_and_unsupported_methods() {
    let manifest = simple_manifest("score.xml");
    for mode in [0o120777, 0o060600, 0o010600] {
        let mut e = Entry::new("score.xml", SCORE);
        e.mode = mode;
        assert_error(
            zip(&[Entry::new(CONTAINER, &manifest), e]),
            "symlink, special",
        );
    }
    for flag in [1, 64, 8192, 16] {
        let mut e = Entry::new("score.xml", SCORE);
        e.flags |= flag;
        assert_error(
            zip(&[Entry::new(CONTAINER, &manifest), e]),
            "unsupported ZIP entry flags",
        );
    }
    let mut e = Entry::new("score.xml", SCORE);
    e.method = 12;
    assert_error(
        zip(&[Entry::new(CONTAINER, &manifest), e]),
        "only stored and DEFLATE",
    );
}
#[test]
fn rejects_invalid_manifests_and_does_not_resolve_entities() {
    for xml in [
        "<?xml version='1.0' foo='bar'?><container/>",
        "<?xml version='1.0' version='1.0'?><container/>",
        "<?xml version='1.0' standalone='perhaps'?><container/>",
        " <!--comment--><?xml version='1.0'?><container/>",
        "<container><!--\0--><rootfiles><rootfile full-path='score.xml'/></rootfiles></container>",
        "<container/>",
        "<container><rootfiles/></container>",
        "<container><rootfiles><rootfile/></rootfiles></container>",
        "<container><rootfiles><rootfile full-path='score.xml' full-path='score.xml'/></rootfiles></container>",
        "<container><rootfiles><rootfile full-path='score.xml'/><rootfile full-path='score.xml'/></rootfiles></container>",
        "<container><rootfiles><rootfile full-path='score.xml'><rootfile full-path='score.xml'/></rootfile></rootfiles></container>",
        "<container><rootfiles><rootfile full-path='score.xml'/></rootfiles></container><container/>",
        "<container><rootfiles><rootfile full-path='score.xml'/></rootfiles>",
        "<!DOCTYPE container SYSTEM 'https://example.invalid/container.dtd'><container><rootfiles><rootfile full-path='score.xml'/></rootfiles></container>",
        "<!DOCTYPE container [<!ENTITY x SYSTEM 'file:///etc/passwd'>]><container><rootfiles><rootfile full-path='&x;'/></rootfiles></container>",
        "<container><rootfiles><rootfile full-path='&unknown;'/></rootfiles></container>",
        "<?xml version='1.0' encoding='UTF-16'?><container/>",
        "<container xmlns='http://external.test'><rootfiles><rootfile full-path='score.xml'/></rootfiles></container>",
        "<container><!-- invalid -- comment --><rootfiles><rootfile full-path='score.xml'/></rootfiles></container>",
        "<container><rootfiles><rootfile full-path='score.xml'/></rootfiles>text</container>",
    ] {
        assert!(
            mxl::open(Cursor::new(archive(xml.as_bytes()))).is_err(),
            "accepted {xml}"
        );
    }
}
#[test]
fn paths_are_exact_and_xml_builtins_are_decoded_once() {
    let manifest = simple_manifest("a&amp;b%20.musicxml");
    let bytes = zip(&[
        Entry::new(CONTAINER, &manifest),
        Entry::new("a&b%20.musicxml", SCORE),
    ]);
    assert_eq!(
        mxl::open(Cursor::new(bytes)).unwrap().root_path(),
        "a&b%20.musicxml"
    );
}
#[test]
fn validates_mimetype_when_present_but_not_required() {
    let manifest = simple_manifest("score.xml");
    let mime = b"application/vnd.recordare.musicxml";
    let bytes = zip(&[
        Entry::new("mimetype", mime).stored(),
        Entry::new(CONTAINER, &manifest),
        Entry::new("score.xml", SCORE),
    ]);
    assert!(mxl::open(Cursor::new(bytes)).is_ok());
    for marker in [
        Entry::new("mimetype", mime),
        Entry::new("mimetype", b"application/zip").stored(),
    ] {
        assert_error(
            zip(&[
                marker,
                Entry::new(CONTAINER, &manifest),
                Entry::new("score.xml", SCORE),
            ]),
            "MIME marker",
        );
    }
}
#[test]
fn rejects_header_disagreements_crc_mismatch_counts_and_overlaps() {
    let original = archive(&simple_manifest("score.xml"));
    let h = headers(&original);
    let mut b = original.clone();
    b[30] = b'X';
    assert_error(b, "headers disagree");
    let mut b = original.clone();
    b[h[1] + 16] ^= 1;
    assert_error(b, "checksum disagree");
    let mut b = original.clone();
    let eocd = b.len() - 22;
    b[eocd + 8..eocd + 12].copy_from_slice(&[3, 0, 3, 0]);
    assert_error(b, "count mismatch");
    let mut b = original.clone();
    b[h[1] + 42..h[1] + 46].copy_from_slice(&0u32.to_le_bytes());
    assert!(mxl::open(Cursor::new(b)).is_err());
    let mut b = original.clone();
    b[h[1] + 20..h[1] + 24].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(mxl::open(Cursor::new(b)).is_err());
    let mut b = original.clone();
    b[eocd + 4] = 1;
    assert_error(b, "split ZIP");
    let mut b = original.clone();
    b[eocd + 12] ^= 1;
    assert_error(b, "extent");
    let mut b = original;
    b.extend(b"trailing");
    assert_error(b, "trailing bytes");
}
#[test]
fn detects_data_crc_and_truncated_deflate_without_trusting_size() {
    let manifest = simple_manifest("score.xml");
    let mut bytes = zip(&[
        Entry::new(CONTAINER, &manifest),
        Entry::new("score.xml", SCORE).stored(),
    ]);
    let h = headers(&bytes);
    let start = u32::from_le_bytes(bytes[h[1] + 42..h[1] + 46].try_into().unwrap()) as usize;
    bytes[start + 30 + "score.xml".len()] ^= 1;
    assert_error(bytes, "CRC integrity");
    let bytes = archive(&manifest);
    for n in 0..bytes.len() {
        assert!(
            mxl::open(Cursor::new(&bytes[..n])).is_err(),
            "accepted prefix {n}"
        );
    }
}
#[test]
fn metadata_limits_are_configurable_and_not_score_size_limits() {
    let bytes = archive(&simple_manifest("score.xml"));
    for limits in [
        ContainerLimits {
            max_entries: 1,
            ..ContainerLimits::default()
        },
        ContainerLimits {
            max_manifest_bytes: 16,
            ..ContainerLimits::default()
        },
        ContainerLimits {
            max_path_bytes: 8,
            ..ContainerLimits::default()
        },
        ContainerLimits {
            max_total_path_bytes: 8,
            ..ContainerLimits::default()
        },
    ] {
        assert!(mxl::open_with_limits(Cursor::new(&bytes), limits).is_err());
    }
    // Greater than the old 1 MiB XML ceiling. Payload is deliberately opaque:
    // the archive layer never performs lossy normalization or semantic parsing.
    let huge = vec![b' '; 8 * 1024 * 1024];
    let bytes = zip(&[
        Entry::new(CONTAINER, &simple_manifest("score.xml")),
        Entry::new("score.xml", &huge),
    ]);
    let root = mxl::open(Cursor::new(bytes)).unwrap();
    assert_eq!(root.xml_bytes(), huge.len() as u64);
}

/// Pinned licensed source corpus lives outside the repository. This optional
/// test never downloads it, and checks root bytes separately from music support.
#[test]
#[ignore = "requires SCORESTREAM_MXL_FIXTURES pointing to the pinned Beethoven corpus"]
fn pinned_beethoven_archives_stream_identical_roots_and_scan_all_movements() {
    use sha2::{Digest, Sha256};
    let directory = std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap();
    for (movement, bytes, sha256) in [
        (
            1,
            5_175_602,
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
        ),
        (
            2,
            4_195_437,
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
        ),
        (
            3,
            3_589_604,
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
        ),
        (
            4,
            10_620_227,
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ),
    ] {
        let file = std::fs::File::open(
            std::path::Path::new(&directory).join(format!("Beethoven_Op.67_{movement}.mxl")),
        )
        .unwrap();
        let root = mxl::open(file).unwrap();
        assert_eq!(root.root_path(), "score.xml");
        assert_eq!(root.xml_bytes(), bytes);
        let mut reader = root.into_reader();
        let mut hasher = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            let count = reader.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        assert_eq!(format!("{:x}", hasher.finalize()), sha256);
        reader.rewind().unwrap();
        let summary = scorestream_formats::musicxml::scan(reader).unwrap();
        println!(
            "movement {movement}: {}",
            serde_json::to_string(&summary).unwrap()
        );
        assert_eq!(summary.bytes, bytes);
    }
}

#[test]
fn manifest_tokenization_does_not_bypass_xml_well_formedness() {
    let body = "<container><rootfiles><rootfile full-path='score.xml'/></rootfiles></container>";
    for prefix in [
        "<?xml version='1.0' foo='bar'?>",
        "<?xml version='1.0' version='1.0'?>",
        "<?xml version='1.0' standalone='perhaps'?>",
        "<?xml version='1.0' standalone='yes' encoding='UTF-8'?>",
        " <?xml version='1.0'?>",
        "<!--comment--><?xml version='1.0'?>",
    ] {
        assert!(mxl::open(Cursor::new(archive(format!("{prefix}{body}").as_bytes()))).is_err());
    }
    let malformed = simple_manifest("score<.xml");
    assert_error(
        zip(&[
            Entry::new(CONTAINER, &malformed),
            Entry::new("score<.xml", SCORE),
        ]),
        "unescaped less-than",
    );
    let valid = simple_manifest("score&lt;.xml");
    assert!(
        mxl::open(Cursor::new(zip(&[
            Entry::new(CONTAINER, &valid),
            Entry::new("score<.xml", SCORE)
        ])))
        .is_ok()
    );
    let valid = format!("\u{feff}<?xml version='1.0' encoding='UTF-8' standalone='yes'?>{body}");
    assert!(mxl::open(Cursor::new(archive(valid.as_bytes()))).is_ok());
}

#[test]
fn manifest_attribute_boundaries_and_decoded_characters_are_valid_xml() {
    let body = "<container><rootfiles><rootfile full-path='score.xml'/></rootfiles></container>";
    assert_error(
        archive(format!("<?xml version='1.0'encoding='UTF-8'?>{body}").as_bytes()),
        "whitespace separators",
    );
    assert_error(
        archive(&manifest(
            "<rootfile full-path='score.xml'media-type='application/vnd.recordare.musicxml+xml'/>",
        )),
        "whitespace separators",
    );
    for entity in ["&#x1;", "&#xFFFE;", "&#xFFFF;"] {
        assert_error(
            archive(&manifest(&format!(
                "<rootfile full-path='score.xml'/><rootfile full-path='alternate.xml' media-type='{entity}'/>"
            ))),
            "decoded XML",
        );
    }
    let invalid_path = simple_manifest("a&#xFFFE;.xml");
    assert_error(
        zip(&[
            Entry::new(CONTAINER, &invalid_path),
            Entry::new("a\u{fffe}.xml", SCORE),
        ]),
        "decoded XML",
    );
}

#[test]
fn selected_stream_must_match_declared_sizes_and_finish_deflate() {
    let original = archive(&simple_manifest("score.xml"));
    let h = headers(&original)[1];
    let local = u32::from_le_bytes(original[h + 42..h + 46].try_into().unwrap()) as usize;
    let declared = u32::from_le_bytes(original[h + 24..h + 28].try_into().unwrap());
    for size in [declared - 1, declared + 1] {
        let mut b = original.clone();
        b[h + 24..h + 28].copy_from_slice(&size.to_le_bytes());
        b[local + 22..local + 26].copy_from_slice(&size.to_le_bytes());
        assert!(mxl::open(Cursor::new(b)).is_err());
    }
    let mut truncated = original;
    let compressed = u32::from_le_bytes(truncated[h + 20..h + 24].try_into().unwrap()) - 3;
    truncated[h + 20..h + 24].copy_from_slice(&compressed.to_le_bytes());
    truncated[local + 18..local + 22].copy_from_slice(&compressed.to_le_bytes());
    assert!(mxl::open(Cursor::new(truncated)).is_err());
}

#[test]
fn rejects_actual_overlapping_ranges_and_corrupt_data_descriptors() {
    let manifest = simple_manifest("score.xml");
    let mut b = zip(&[
        Entry::new(CONTAINER, &manifest),
        Entry::new("overlap.bin", b"").stored(),
        Entry::new("score.xml", SCORE),
    ]);
    let h = headers(&b);
    let local = u32::from_le_bytes(b[h[1] + 42..h[1] + 46].try_into().unwrap()) as usize;
    let data = local + 30 + "overlap.bin".len();
    let size = (h[0] - data) as u32;
    for at in [h[1] + 20, h[1] + 24, local + 18, local + 22] {
        b[at..at + 4].copy_from_slice(&size.to_le_bytes());
    }
    assert_error(b, "overlapping");
    let mut root = Entry::new("score.xml", SCORE);
    root.descriptor = true;
    let mut b = zip(&[Entry::new(CONTAINER, &manifest), root]);
    let descriptor = b.windows(4).position(|w| w == b"PK\x07\x08").unwrap();
    b[descriptor + 4] ^= 1;
    assert_error(b, "descriptor disagrees");
}
