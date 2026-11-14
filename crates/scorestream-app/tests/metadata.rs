use scorestream_app::Workspace;
use serde_json::json;

const SOURCE: &str = include_str!("../../../design/examples/sfs/document-metadata.sfs");

#[test]
fn desktop_view_and_committed_snapshot_use_the_same_typed_metadata() {
    let workspace = Workspace::from_source(SOURCE).unwrap();
    let view = workspace.current();
    assert!(view.valid);
    assert_eq!(view.document["metadata"]["title"], "春日练习");
    assert_eq!(
        view.document["metadata"]["authors"],
        json!(["林青", "Alex Chen"])
    );
    assert_eq!(
        workspace.committed_snapshot().document["metadata"],
        view.document["metadata"]
    );
    assert!(view.document["metadata"].get("source").is_none());
}

#[test]
fn metadata_draft_commit_and_history_share_existing_revision_authority() {
    let mut workspace = Workspace::from_source(SOURCE).unwrap();
    let initial = workspace.current();
    let changed = SOURCE.replace("春日练习", "夜曲");
    let preview = workspace.preview_source(&changed);
    assert!(preview.valid);
    assert_eq!(preview.document["metadata"]["title"], "夜曲");
    assert_eq!(
        workspace.committed_snapshot().document["metadata"]["title"],
        "春日练习"
    );
    let committed = workspace
        .apply_source(&changed, &initial.revision_id)
        .unwrap();
    assert_eq!(committed.document["metadata"]["title"], "夜曲");
    assert_eq!(
        workspace.undo().unwrap().document["metadata"]["title"],
        "春日练习"
    );
    assert_eq!(
        workspace.redo().unwrap().document["metadata"]["title"],
        "夜曲"
    );
    let bad = changed.replace(r#"["林青", "Alex Chen"]"#, "[3]");
    let invalid = workspace.preview_source(&bad);
    assert!(!invalid.valid);
    assert_eq!(invalid.document["metadata"]["title"], "夜曲");
    assert_eq!(
        workspace.committed_snapshot().document["metadata"]["authors"],
        json!(["林青", "Alex Chen"])
    );
}
