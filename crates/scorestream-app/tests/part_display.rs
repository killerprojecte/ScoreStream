use scorestream_app::Workspace;

#[test]
fn typed_preview_and_canonical_edits_keep_author_labels() {
    let source = "!score 1.1 unit=q\n!part p instrument=flute staves=1 display_name=\"Flute 2 & solo\"\n!voice v part=p staff=1\n@0\n| p m len=1\nv C4:1 id=n\n.\n!end at=1\n";
    let mut workspace = Workspace::from_source(source).unwrap();
    let current = workspace.current();
    assert_eq!(current.parts[0]["display_name"], "Flute 2 & solo");
    assert_eq!(
        current.document["parts"][0]["display_name"],
        "Flute 2 & solo"
    );
    let edited = workspace
        .set_event_pitch("n", "D4", &current.revision_id)
        .unwrap();
    assert!(edited.source.contains("display_name=\"Flute 2 & solo\""));
    assert_eq!(edited.parts[0]["instrument"], "flute");
    assert_eq!(edited.parts[0]["display_name"], "Flute 2 & solo");
    let restored = workspace.undo().unwrap();
    assert_eq!(restored.parts[0]["display_name"], "Flute 2 & solo");
}
