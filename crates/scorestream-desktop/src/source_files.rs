//! Native file conversion is completed before replacing the active workspace.
use scorestream_app::{ViewResponse, Workspace};
use scorestream_formats::{MappingReport, ScoreAdapter, musicxml, mxl, sfs::SfsAdapter};
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

#[derive(serde::Serialize)]
pub struct OpenedFile {
    pub import_token: Option<String>,
    pub path: String,
    pub source: String,
    pub imported_view: Option<ViewResponse>,
    pub import_report: MappingReport,
}
pub struct PreparedFile {
    pub opened: OpenedFile,
    pub workspace: Option<Workspace>,
    pub origin: Option<PathBuf>,
}
pub type Pending = Option<(String, Workspace, PathBuf)>;
pub fn activate(
    prepared: &mut Pending,
    workspace: &mut Workspace,
    imported: &mut Option<PathBuf>,
    token: &str,
    expected_revision_id: &str,
) -> Result<ViewResponse, String> {
    if workspace.current().revision_id != expected_revision_id {
        return Err("The score changed while importing. Open the file again.".into());
    }
    if !prepared.as_ref().is_some_and(|item| item.0 == token) {
        return Err("The import is no longer available. Open the file again.".into());
    }
    let (_, replacement, path) = prepared.take().unwrap();
    *workspace = replacement;
    *imported = Some(path);
    Ok(workspace.current())
}
fn report_error(report: MappingReport) -> String {
    serde_json::to_string(&report).unwrap_or_else(|_| "Cannot describe import failure".into())
}
pub fn read(path: &Path) -> Result<PreparedFile, String> {
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "sfs" | "xml" | "musicxml" | "mxl") {
        return Err("Choose an SFS, MusicXML, XML or MXL file.".into());
    }
    if !std::fs::metadata(path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("Choose a regular score file.".into());
    }
    let file = File::open(path).map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("Choose a regular score file.".into());
    }
    if extension == "sfs" {
        if metadata.len() > 4 * 1024 * 1024 {
            return Err("This editor slice accepts SFS files up to 4 MiB.".into());
        }
        let source = std::io::read_to_string(file).map_err(|e| e.to_string())?;
        return Ok(PreparedFile {
            opened: OpenedFile {
                import_token: None,
                path: path.display().to_string(),
                source,
                imported_view: None,
                import_report: MappingReport::default(),
            },
            workspace: None,
            origin: None,
        });
    }
    let options = musicxml::MusicXmlOptions {
        articulations: musicxml::ArticulationProfile::Automatic,
        ..Default::default()
    };
    let converted = if extension == "mxl" {
        let root = mxl::open(file).map_err(report_error)?;
        musicxml::read_reader_with_options(root.into_reader(), &options)
    } else {
        musicxml::read_reader_with_options(BufReader::new(file), &options)
    }
    .map_err(report_error)?;
    let written = SfsAdapter.write(&converted.value).map_err(report_error)?;
    let mut report = converted.report;
    report.append(written.report);
    let workspace = Workspace::from_source(&written.value)?;
    let view = workspace.current();
    let origin = path.canonicalize().map_err(|e| e.to_string())?;
    Ok(PreparedFile {
        opened: OpenedFile {
            import_token: None,
            path: path.display().to_string(),
            source: written.value,
            imported_view: Some(view),
            import_report: report,
        },
        workspace: Some(workspace),
        origin: Some(origin),
    })
}
/// Picker filters are hints, not an overwrite policy.
pub fn validate_destination(path: &Path, imported: Option<&Path>) -> Result<(), String> {
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("sfs"))
    {
        return Err("Save the converted score as a new .sfs file; the original MusicXML is not overwritten.".into());
    }
    if let Some(origin) = imported {
        if path.exists() && same_file::is_same_file(origin, path).map_err(|e| e.to_string())? {
            return Err("Choose a different destination from the imported source file.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(path: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../scorestream-formats/tests/fixtures")
            .join(path)
    }
    #[test]
    fn activation_is_revision_checked_single_use_and_transactional() {
        let prepared = read(&fixture("two-staff.musicxml")).unwrap();
        let mut current = Workspace::demo().unwrap();
        let before = current.current();
        let mut pending = Some((
            "token".into(),
            prepared.workspace.unwrap(),
            prepared.origin.unwrap(),
        ));
        let mut origin = None;
        assert!(activate(&mut pending, &mut current, &mut origin, "token", "stale").is_err());
        assert!(
            activate(
                &mut pending,
                &mut current,
                &mut origin,
                "wrong",
                &before.revision_id
            )
            .is_err()
        );
        assert_eq!(before.source, current.current().source);
        assert!(origin.is_none());
        assert!(pending.is_some());
        let new = activate(
            &mut pending,
            &mut current,
            &mut origin,
            "token",
            &before.revision_id,
        )
        .unwrap();
        assert_eq!(new.source, prepared.opened.source);
        assert!(origin.is_some());
        assert!(pending.is_none());
        assert!(!new.can_undo);
        assert!(
            activate(
                &mut pending,
                &mut current,
                &mut origin,
                "token",
                &new.revision_id
            )
            .is_err()
        );
    }
    #[test]
    fn xml_and_mxl_prepare_equivalent_unsaved_workspaces() {
        let xml = read(&fixture("two-staff.musicxml")).unwrap();
        let mxl = read(&fixture("two-staff.mxl")).unwrap();
        assert!(xml.workspace.is_some());
        assert!(xml.opened.imported_view.as_ref().unwrap().valid);
        assert_eq!(xml.opened.source, mxl.opened.source);
        assert_eq!(
            xml.opened.source,
            xml.workspace.as_ref().unwrap().current().source
        );
    }
    #[test]
    fn rich_import_keeps_reports_and_valid_sfs_without_claiming_audio() {
        let rich = read(&fixture("mixed-authored-notations.musicxml")).unwrap();
        assert!(
            rich.opened
                .import_report
                .diagnostics
                .iter()
                .any(|d| d.feature == "articulation.automatic_retention")
        );
        assert!(rich.opened.source.contains("scorestream.notations@1"));
        assert!(rich.opened.imported_view.unwrap().valid);
    }
    #[test]
    fn malformed_or_unsupported_conversion_produces_no_workspace() {
        assert!(read(&fixture("unsupported.musicxml")).is_err());
        let temp = tempfile::tempdir().unwrap();
        let bad = temp.path().join("bad.mxl");
        std::fs::write(&bad, b"not a ZIP").unwrap();
        assert!(read(&bad).is_err());
        let bad = temp.path().join("bad.xml");
        std::fs::write(&bad, b"<score-partwise>").unwrap();
        assert!(read(&bad).is_err());
    }
    #[test]
    fn sfs_preserves_the_existing_draft_route_even_for_invalid_source() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.sfs");
        std::fs::write(&path, b"invalid draft").unwrap();
        let result = read(&path).unwrap();
        assert!(result.workspace.is_none());
        assert!(result.opened.imported_view.is_none());
        assert_eq!(result.opened.source, "invalid draft");
    }
    #[test]
    fn save_rejects_source_extensions_and_hard_link_aliases() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.xml");
        std::fs::write(&source, b"original").unwrap();
        let alias = temp.path().join("alias.sfs");
        std::fs::hard_link(&source, &alias).unwrap();
        assert!(validate_destination(&source, Some(&source)).is_err());
        assert!(validate_destination(&alias, Some(&source)).is_err());
        assert!(validate_destination(&temp.path().join("new.sfs"), Some(&source)).is_ok());
        assert_eq!(std::fs::read(&source).unwrap(), b"original");
    }
    #[cfg(unix)]
    #[test]
    fn save_rejects_symlink_aliases() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.mxl");
        std::fs::write(&source, b"original").unwrap();
        let alias = temp.path().join("alias.sfs");
        std::os::unix::fs::symlink(&source, &alias).unwrap();
        assert!(validate_destination(&alias, Some(&source)).is_err());
    }
}
