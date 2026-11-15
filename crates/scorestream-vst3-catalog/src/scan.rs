use crate::{
    CatalogError, ClassId, HeaderInfo, ModuleInfo, Result, digest, inspect_header, parse_moduleinfo,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct ScanLimits {
    pub max_roots: usize,
    pub max_entries: usize,
    pub max_candidates: usize,
    pub max_depth: usize,
    pub max_manifest_bytes: usize,
    pub max_header_bytes: usize,
    pub max_total_file_bytes: usize,
}
impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            max_roots: 32,
            max_entries: 20_000,
            max_candidates: 1000,
            max_depth: 8,
            max_manifest_bytes: 1024 * 1024,
            max_header_bytes: 65536,
            max_total_file_bytes: 32 * 1024 * 1024,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub code: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct BinaryCandidate {
    pub path: PathBuf,
    pub layout_hint: String,
    pub file_size: u64,
    pub header_bytes_read: usize,
    /// Hash of inspected prefix only, explicitly NOT a whole-module identity.
    pub header_prefix_sha256: String,
    pub header: HeaderInfo,
}
#[derive(Clone, Debug, Serialize)]
pub struct Candidate {
    pub path: PathBuf,
    pub root_priority: usize,
    pub legacy_single_file: bool,
    pub manifest_path: Option<PathBuf>,
    /// Identifies manifest bytes only. Plugin code must be fingerprinted later.
    pub manifest_sha256: Option<String>,
    pub metadata: Option<ModuleInfo>,
    pub binaries: Vec<BinaryCandidate>,
    pub status: String,
    pub host_verified: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct DuplicateClass {
    pub cid: ClassId,
    pub first_candidate: usize,
    pub later_candidates: Vec<usize>,
}
#[derive(Debug, Serialize)]
pub struct ScanReport {
    pub schema: String,
    pub candidates: Vec<Candidate>,
    pub diagnostics: Vec<Diagnostic>,
    pub duplicate_classes: Vec<DuplicateClass>,
    pub inspected_entries: usize,
    pub retained_file_bytes: usize,
    pub complete: bool,
    pub executes_plugin_code: bool,
}

struct Scanner<'a> {
    limits: &'a ScanLimits,
    report: ScanReport,
    seen_candidates: BTreeSet<PathBuf>,
}
impl Scanner<'_> {
    fn read_file(
        &mut self,
        root: &Path,
        path: &Path,
        limit: usize,
        prefix: bool,
    ) -> Result<(Vec<u8>, u64)> {
        let remaining = self.limits.max_total_file_bytes - self.report.retained_file_bytes;
        if remaining == 0 {
            self.report.complete = false;
            return Err(CatalogError::Limit(
                "Total file-byte budget exhausted".into(),
            ));
        }
        let result = read_regular(root, path, limit.min(remaining), prefix);
        if let Ok((ref bytes, size)) = result {
            self.report.retained_file_bytes += bytes.len();
            let required = if prefix { size.min(limit as u64) } else { size };
            if (bytes.len() as u64) < required {
                self.report.complete = false;
            }
        } else if matches!(result, Err(CatalogError::Limit(_))) && remaining < limit {
            self.report.complete = false;
        }
        result
    }
    fn diagnostic(&mut self, path: &Path, code: &str, message: impl ToString) {
        self.report.diagnostics.push(Diagnostic {
            path: path.into(),
            code: code.into(),
            message: message.to_string(),
        });
    }
    fn entries(&mut self, directory: &Path) -> Result<Vec<PathBuf>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(directory)? {
            if self.report.inspected_entries == self.limits.max_entries {
                return Err(CatalogError::Limit(
                    "Filesystem entry budget exhausted".into(),
                ));
            }
            self.report.inspected_entries += 1;
            entries.push(entry?.path());
        }
        entries.sort();
        Ok(entries)
    }
    fn walk(&mut self, root: &Path, directory: &Path, priority: usize, depth: usize) -> Result<()> {
        for path in self.entries(directory)? {
            let metadata = match checked_metadata(root, &path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    self.diagnostic(&path, "entry-skipped", error);
                    continue;
                }
            };
            if is_vst3(&path) && (metadata.is_dir() || metadata.is_file()) {
                if !self.seen_candidates.insert(path.clone()) {
                    self.diagnostic(
                        &path,
                        "duplicate-candidate",
                        "Overlapping roots reached an already inspected candidate",
                    );
                    continue;
                }
                if self.report.candidates.len() == self.limits.max_candidates {
                    return Err(CatalogError::Limit("Candidate budget exhausted".into()));
                }
                let candidate = self.inspect(root, &path, priority, metadata.is_file())?;
                self.report.candidates.push(candidate);
            } else if metadata.is_dir() {
                if depth >= self.limits.max_depth {
                    self.report.complete = false;
                    self.diagnostic(
                        &path,
                        "depth-limited",
                        "Directory was not inspected beyond the explicit depth budget",
                    );
                } else {
                    self.walk(root, &path, priority, depth + 1)?;
                }
            }
        }
        Ok(())
    }
    fn inspect(
        &mut self,
        root: &Path,
        path: &Path,
        priority: usize,
        single: bool,
    ) -> Result<Candidate> {
        let mut candidate = Candidate {
            path: path.into(),
            root_priority: priority,
            legacy_single_file: single,
            manifest_path: None,
            manifest_sha256: None,
            metadata: None,
            binaries: Vec::new(),
            status: "metadata_missing_requires_future_isolated_scan".into(),
            host_verified: false,
        };
        if single {
            self.binary(root, path, "legacy-single-file", &mut candidate);
            self.diagnostic(
                path,
                "legacy-manifest-unavailable",
                "Single-file candidates have no bundle manifest; no execution attempted",
            );
            return Ok(candidate);
        }
        let modern = path.join("Contents/Resources/moduleinfo.json");
        let legacy = path.join("Contents/moduleinfo.json");
        // A malformed modern manifest must not silently fall back to older data.
        let selected = match checked_metadata(root, &modern) {
            Ok(_) => Some(modern),
            Err(CatalogError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                match checked_metadata(root, &legacy) {
                    Ok(_) => Some(legacy),
                    Err(CatalogError::Io(error))
                        if error.kind() == std::io::ErrorKind::NotFound =>
                    {
                        None
                    }
                    Err(error) => {
                        candidate.status = "metadata_invalid".into();
                        self.diagnostic(&legacy, "manifest-unreadable", error);
                        None
                    }
                }
            }
            Err(error) => {
                candidate.status = "metadata_invalid".into();
                self.diagnostic(&modern, "manifest-unreadable", error);
                None
            }
        };
        if let Some(manifest_path) = selected {
            candidate.manifest_path = Some(manifest_path.clone());
            candidate.status = "metadata_invalid".into();
            match self.read_file(root, &manifest_path, self.limits.max_manifest_bytes, false) {
                Ok((bytes, _)) => {
                    candidate.manifest_sha256 = Some(digest(&bytes));
                    match std::str::from_utf8(&bytes)
                        .map_err(|_| CatalogError::Invalid("Manifest is not UTF-8".into()))
                        .and_then(parse_moduleinfo)
                    {
                        Ok(metadata) => {
                            candidate.metadata = Some(metadata);
                            candidate.status = "metadata_only_unverified".into();
                        }
                        Err(error) => self.diagnostic(&manifest_path, "manifest-invalid", error),
                    }
                }
                Err(error) => self.diagnostic(&manifest_path, "manifest-unreadable", error),
            }
        } else if candidate.status == "metadata_missing_requires_future_isolated_scan" {
            self.diagnostic(path, "manifest-missing", "Optional moduleinfo is absent; classes cannot be known without a future isolated runtime scan");
        }
        let contents = path.join("Contents");
        if checked_metadata(root, &contents).is_ok_and(|metadata| metadata.is_dir()) {
            for directory in self.entries(&contents)? {
                let Some(name) = directory.file_name().and_then(|value| value.to_str()) else {
                    continue;
                };
                if !(name == "MacOS" || name.ends_with("-linux") || name.ends_with("-win")) {
                    continue;
                }
                match checked_metadata(root, &directory) {
                    Ok(metadata) if metadata.is_dir() => {}
                    Ok(_) => continue,
                    Err(error) => {
                        self.diagnostic(&directory, "binary-directory-skipped", error);
                        continue;
                    }
                }
                if name == "MacOS" {
                    for binary in self.entries(&directory)? {
                        self.binary(root, &binary, name, &mut candidate);
                    }
                } else if let Some(stem) = path.file_stem() {
                    let mut filename = stem.to_os_string();
                    filename.push(if name.ends_with("-win") {
                        ".vst3"
                    } else {
                        ".so"
                    });
                    self.binary(root, &directory.join(filename), name, &mut candidate);
                }
            }
        }
        if candidate.binaries.is_empty() {
            self.diagnostic(
                path,
                "no-recognized-binary",
                "No bounded executable header was recognized in standard bundle locations",
            );
        }
        Ok(candidate)
    }
    fn binary(&mut self, root: &Path, path: &Path, hint: &str, candidate: &mut Candidate) {
        match self.read_file(root, path, self.limits.max_header_bytes, true) {
            Ok((bytes, size)) => match inspect_header(&bytes, size) {
                Ok(header) => {
                    let expected_format = if hint.ends_with("-linux") {
                        "elf"
                    } else if hint.ends_with("-win") {
                        "pe"
                    } else if hint == "MacOS" {
                        "mach"
                    } else {
                        "any"
                    };
                    let actual = match header.format {
                        crate::BinaryFormat::Elf => "elf",
                        crate::BinaryFormat::Pe => "pe",
                        _ => "mach",
                    };
                    if expected_format != "any" && expected_format != actual {
                        self.diagnostic(path, "layout-header-conflict", "Directory platform hint disagrees with the actual header; no compatibility inference made");
                    }
                    let expected_arch = match hint {
                        "x86-win" | "i386-linux" | "i686-linux" => Some(crate::Architecture::X86),
                        "x86_64-win" | "x86_64-linux" => Some(crate::Architecture::X86_64),
                        "arm64-win" | "aarch64-linux" => Some(crate::Architecture::Arm64),
                        "arm64ec-win" => Some(crate::Architecture::Arm64Ec),
                        "arm64x-win" => Some(crate::Architecture::Arm64X),
                        "arm-win" => Some(crate::Architecture::Arm),
                        _ => None,
                    };
                    if expected_arch.is_some_and(|arch| !header.architectures.contains(&arch)) {
                        self.diagnostic(path, "layout-architecture-conflict", "Directory architecture hint disagrees with the actual header; candidate is not load-verified");
                    }
                    candidate.binaries.push(BinaryCandidate {
                        path: path.into(),
                        layout_hint: hint.into(),
                        file_size: size,
                        header_bytes_read: bytes.len(),
                        header_prefix_sha256: digest(&bytes),
                        header,
                    });
                }
                Err(error) => self.diagnostic(path, "binary-header-invalid", error),
            },
            Err(error) => self.diagnostic(path, "binary-unreadable", error),
        }
    }
}

fn is_vst3(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("vst3"))
}
fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0 // Includes directory junctions/reparse points.
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn checked_metadata(root: &Path, path: &Path) -> Result<fs::Metadata> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| CatalogError::Invalid("Path outside explicit root".into()))?;
    let mut current = root.to_path_buf();
    for part in relative.components() {
        if !matches!(part, Component::Normal(_)) {
            return Err(CatalogError::Invalid("Nonlocal path component".into()));
        }
        current.push(part);
        if is_link(&fs::symlink_metadata(&current)?) {
            return Err(CatalogError::Invalid(
                "Symlink/reparse-point traversal is unsupported in metadata-only discovery".into(),
            ));
        }
    }
    Ok(fs::symlink_metadata(path)?)
}
fn read_regular(root: &Path, path: &Path, limit: usize, prefix: bool) -> Result<(Vec<u8>, u64)> {
    let before = checked_metadata(root, path)?;
    if !before.is_file() {
        return Err(CatalogError::Invalid("Not a regular file".into()));
    }
    if !prefix && before.len() > limit as u64 {
        return Err(CatalogError::Limit(
            "Manifest exceeds configured byte budget".into(),
        ));
    }
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    if !opened.is_file()
        || opened.len() != before.len()
        || opened.modified()? != before.modified()?
    {
        return Err(CatalogError::Invalid("File changed before read".into()));
    }
    let count = if prefix { limit } else { limit + 1 };
    let mut bytes = Vec::new();
    (&mut file).take(count as u64).read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if after.len() != opened.len()
        || after.modified()? != opened.modified()?
        || (!prefix && bytes.len() > limit)
    {
        return Err(CatalogError::Invalid(
            "File changed during bounded read".into(),
        ));
    }
    Ok((bytes, after.len()))
}

/// Explicit caller-approved local roots only. No defaults are opened; no library,
/// plug-in, manifest URL, snapshot path, helper process or native factory is run.
/// Stable local trees are required; this is not an adversarial filesystem sandbox.
pub fn scan_roots(roots: &[PathBuf], limits: &ScanLimits) -> Result<ScanReport> {
    if roots.len() > limits.max_roots
        || limits.max_roots > 64
        || limits.max_entries == 0
        || limits.max_entries > 100_000
        || limits.max_candidates == 0
        || limits.max_candidates > 10_000
        || limits.max_depth > 32
        || limits.max_manifest_bytes == 0
        || limits.max_manifest_bytes > 1024 * 1024
        || limits.max_header_bytes < 64
        || limits.max_header_bytes > 65536
        || limits.max_total_file_bytes == 0
        || limits.max_total_file_bytes > 128 * 1024 * 1024
    {
        return Err(CatalogError::Limit(
            "Invalid/excessive discovery budget".into(),
        ));
    }
    let mut scanner = Scanner {
        limits,
        seen_candidates: BTreeSet::new(),
        report: ScanReport {
            schema: "scorestream.vst3-metadata.v1".into(),
            candidates: Vec::new(),
            diagnostics: Vec::new(),
            duplicate_classes: Vec::new(),
            inspected_entries: 0,
            retained_file_bytes: 0,
            complete: true,
            executes_plugin_code: false,
        },
    };
    let mut seen = BTreeSet::new();
    for (priority, root) in roots.iter().enumerate() {
        if !root.is_absolute() || root.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err(CatalogError::Invalid(
                "Discovery roots must be explicit absolute paths".into(),
            ));
        }
        let checked = fs::symlink_metadata(root).and_then(|metadata| {
            if !metadata.is_dir() || is_link(&metadata) {
                return Err(std::io::Error::other("Root is not a real directory"));
            }
            root.canonicalize()
        });
        let root = match checked {
            Ok(root) => root,
            Err(error) => {
                scanner.report.complete = false;
                scanner.diagnostic(root, "root-unavailable", error);
                continue;
            }
        };
        if !seen.insert(root.clone()) {
            scanner.diagnostic(
                &root,
                "duplicate-root",
                "Canonical root was already inspected",
            );
            continue;
        }
        if let Err(error) = scanner.walk(&root, &root, priority, 0) {
            scanner.report.complete = false;
            let exhausted = matches!(error, CatalogError::Limit(_));
            scanner.diagnostic(
                &root,
                if exhausted {
                    "budget-exhausted"
                } else {
                    "root-incomplete"
                },
                error,
            );
            if exhausted {
                break;
            }
        }
    }
    let mut locations: BTreeMap<ClassId, Vec<usize>> = BTreeMap::new();
    for (index, candidate) in scanner.report.candidates.iter().enumerate() {
        if let Some(metadata) = &candidate.metadata {
            for class in &metadata.classes {
                if class.category == "Audio Module Class" {
                    locations.entry(class.cid.clone()).or_default().push(index);
                }
            }
        }
    }
    scanner.report.duplicate_classes = locations
        .into_iter()
        .filter_map(|(cid, indices)| {
            (indices.len() > 1).then(|| DuplicateClass {
                cid,
                first_candidate: indices[0],
                later_candidates: indices[1..].to_vec(),
            })
        })
        .collect();
    Ok(scanner.report)
}
