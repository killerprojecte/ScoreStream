//! Shared application authority for the first single-process desktop milestone.
//! Private drafts never enter committed history. Every accepted history action
//! receives a fresh revision, including undo/redo, so stale edits stay stale.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const DEMO_SOURCE: &str = include_str!("../../../examples/first-light.sfs");

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ViewDiagnostic {
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub code: String,
    pub severity: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViewResponse {
    pub valid: bool,
    pub diagnostics: Vec<ViewDiagnostic>,
    pub source: String,
    pub events: Vec<Value>,
    pub parts: Vec<Value>,
    pub voices: Vec<Value>,
    pub measures: Vec<Value>,
    pub states: Vec<Value>,
    pub document: Value,
    pub revision_id: String,
    pub dirty: bool,
    pub draft_seq: u64,
    pub can_undo: bool,
    pub can_redo: bool,
}
#[derive(Clone)]
struct Snapshot {
    source: String,
    document: Value,
    locations: BTreeMap<String, usize>,
    warnings: Vec<ViewDiagnostic>,
}

pub struct Workspace {
    history: Vec<Snapshot>,
    cursor: usize,
    revision_counter: u64,
    revision_id: String,
    draft_seq: u64,
    last_good: Snapshot,
    buffer_dirty: bool,
}

fn str_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .into()
}
fn parse_snapshot(source: &str) -> Result<Snapshot, Vec<ViewDiagnostic>> {
    match scorestream_codec::parse(source) {
        Ok(document) => {
            let value = serde_json::to_value(&document).expect("serializable typed AST");
            if value.get("document_kind").and_then(Value::as_str) == Some("fragment") {
                return Err(vec![ViewDiagnostic {
                    line: 1,
                    column: 1,
                    message:
                        "Open a complete score. Fragments require an explicit bound edit context."
                            .into(),
                    code: "FRAGMENT_CONTEXT_REQUIRED".into(),
                    severity: "error".into(),
                }]);
            }
            let locations = document
                .events()
                .filter_map(|(_, e)| e.id().map(|id| (id.to_string(), e.source.line)))
                .collect();
            let warnings = document.diagnostics.iter().map(view_diagnostic).collect();
            Ok(Snapshot {
                source: source.into(),
                document: value,
                locations,
                warnings,
            })
        }
        Err(diagnostics) => Err(diagnostics.iter().map(view_diagnostic).collect()),
    }
}
fn view_diagnostic(d: &scorestream_codec::Diagnostic) -> ViewDiagnostic {
    ViewDiagnostic {
        line: d.source.line,
        column: d.source.column,
        message: d.message.clone(),
        code: d.code.clone(),
        severity: serde_json::to_value(d.severity)
            .unwrap()
            .as_str()
            .unwrap_or("error")
            .into(),
    }
}
fn projection(snapshot: &Snapshot) -> (Vec<Value>, Vec<Value>, Vec<Value>) {
    let doc = &snapshot.document;
    let mut events = Vec::new();
    let mut measures = Vec::new();
    let mut states = Vec::new();
    let mut voice_staff = std::collections::BTreeMap::<String, u64>::new();
    if let Some(voices) = doc["voices"].as_array() {
        for voice in voices {
            voice_staff.insert(str_field(voice, "id"), voice["staff"].as_u64().unwrap_or(1));
        }
    }
    if let Some(frames) = doc["frames"].as_array() {
        for frame in frames {
            let Some(records) = frame["records"].as_array() else {
                continue;
            };
            // State is frame-wide, regardless of source-line order.
            for record in records {
                if record["record"] == "state" {
                    let scope = str_field(record, "target");
                    if let (Some(staff), Some(current)) = (
                        record["attributes"]["staff"].as_u64(),
                        voice_staff.get_mut(&scope),
                    ) {
                        *current = staff;
                    }
                }
            }
            for record in records {
                let attrs = &record["attributes"];
                match record["record"].as_str() {
                    Some("event") => {
                        let voice = str_field(record,"voice");
                        let staff = attrs["staff"].as_u64().unwrap_or(*voice_staff.get(&voice).unwrap_or(&1));
                        events.push(json!({"id": attrs["id"], "voice": voice, "pitch": record["pitch"],
                            "time": frame["time"], "duration": record["duration"], "staff":staff,
                            "chord":attrs["chord"], "written_duration":attrs["w"], "dots":attrs["dots"].as_u64().unwrap_or(0),
                            "source_line": snapshot.locations.get(attrs["id"].as_str().unwrap_or("")), "attributes":attrs}));
                    }
                    Some("state") => {
                        let scope = str_field(record,"target");
                        states.push(json!({"time":frame["time"],"scope":scope,"values":attrs}));
                    }
                    Some("measure") => measures.push(json!({"id":record["id"],"part":record["target"],"time":frame["time"],"length":record["length"],"label":attrs["label"]})),
                    _ => (),
                }
            }
        }
    }
    (events, measures, states)
}
impl Workspace {
    pub fn demo() -> Result<Self, String> {
        Self::from_source(DEMO_SOURCE)
    }
    pub fn from_source(source: &str) -> Result<Self, String> {
        let snapshot = parse_snapshot(source).map_err(|d| serde_json::to_string(&d).unwrap())?;
        let mut workspace = Self {
            history: vec![snapshot.clone()],
            cursor: 0,
            revision_counter: 0,
            revision_id: String::new(),
            draft_seq: 0,
            last_good: snapshot,
            buffer_dirty: false,
        };
        workspace.next_revision();
        Ok(workspace)
    }
    fn next_revision(&mut self) {
        self.revision_counter += 1;
        let digest = Sha256::digest(self.history[self.cursor].source.as_bytes());
        self.revision_id = format!("rev-{}-{:x}", self.revision_counter, digest)[..26].into();
    }
    fn view(
        &self,
        snapshot: &Snapshot,
        source: &str,
        valid: bool,
        diagnostics: Vec<ViewDiagnostic>,
    ) -> ViewResponse {
        let (events, measures, states) = projection(snapshot);
        let diagnostics = if valid {
            snapshot.warnings.clone()
        } else {
            diagnostics
        };
        ViewResponse {
            valid,
            diagnostics,
            source: source.into(),
            events,
            measures,
            states,
            parts: snapshot.document["parts"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            voices: snapshot.document["voices"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            document: snapshot.document.clone(),
            revision_id: self.revision_id.clone(),
            dirty: source != self.history[self.cursor].source,
            draft_seq: self.draft_seq,
            can_undo: self.cursor > 0,
            can_redo: self.cursor + 1 < self.history.len(),
        }
    }
    pub fn current(&self) -> ViewResponse {
        let snap = &self.history[self.cursor];
        self.view(snap, &snap.source, true, vec![])
    }
    pub fn preview_source(&mut self, source: &str) -> ViewResponse {
        self.draft_seq += 1;
        self.buffer_dirty = source != self.history[self.cursor].source;
        match parse_snapshot(source) {
            Ok(snapshot) => {
                self.last_good = snapshot;
                self.view(&self.last_good, source, true, vec![])
            }
            Err(diagnostics) => self.view(&self.last_good, source, false, diagnostics),
        }
    }
    pub fn apply_source(
        &mut self,
        source: &str,
        expected_revision_id: &str,
    ) -> Result<ViewResponse, String> {
        if expected_revision_id != self.revision_id {
            return Err(format!(
                "REVISION_CONFLICT: expected {expected_revision_id}, current {}. Your text has not been changed.",
                self.revision_id
            ));
        }
        let snapshot = parse_snapshot(source).map_err(|d| serde_json::to_string(&d).unwrap())?;
        if source != self.history[self.cursor].source {
            self.history.truncate(self.cursor + 1);
            self.history.push(snapshot.clone());
            self.cursor += 1;
            self.next_revision();
        }
        self.last_good = snapshot;
        self.buffer_dirty = false;
        self.draft_seq += 1;
        Ok(self.current())
    }
    pub fn set_event_pitch(
        &mut self,
        event_id: &str,
        pitch: &str,
        expected_revision_id: &str,
    ) -> Result<ViewResponse, String> {
        let pitch: scorestream_codec::Pitch = pitch.parse()?;
        self.edit_event(event_id, expected_revision_id, move |event| {
            event.pitch = pitch;
        })
    }
    pub fn set_event_duration(
        &mut self,
        event_id: &str,
        duration: &str,
        expected_revision_id: &str,
    ) -> Result<ViewResponse, String> {
        let duration: scorestream_codec::Duration = duration
            .parse::<scorestream_codec::Duration>()
            .map_err(|e| e.to_string())?;
        self.edit_event(event_id, expected_revision_id, move |event| {
            event.duration = duration;
        })
    }
    fn edit_event(
        &mut self,
        event_id: &str,
        expected_revision_id: &str,
        edit: impl FnOnce(&mut scorestream_codec::Event),
    ) -> Result<ViewResponse, String> {
        if self.buffer_dirty {
            return Err("Apply or discard the dirty source buffer before editing notation".into());
        }
        if expected_revision_id != self.revision_id {
            return Err("REVISION_CONFLICT: re-read the committed score before editing".into());
        }
        let mut doc = scorestream_codec::parse(&self.history[self.cursor].source)
            .map_err(|_| "Committed snapshot did not validate")?;
        let event = doc
            .frames
            .iter_mut()
            .flat_map(|f| f.records.iter_mut())
            .find_map(|r| match r {
                scorestream_codec::Record::Event(e) if e.id() == Some(event_id) => Some(e),
                _ => None,
            })
            .ok_or("Unknown event identity")?;
        edit(event);
        let candidate = scorestream_codec::canonical(&doc);
        self.apply_source(&candidate, expected_revision_id)
    }
    pub fn undo(&mut self) -> Result<ViewResponse, String> {
        if self.buffer_dirty {
            return Err("Apply or discard the dirty source buffer before undo".into());
        }
        if self.cursor == 0 {
            return Err("Nothing to undo".into());
        }
        self.cursor -= 1;
        self.next_revision();
        self.last_good = self.history[self.cursor].clone();
        self.draft_seq += 1;
        Ok(self.current())
    }
    pub fn redo(&mut self) -> Result<ViewResponse, String> {
        if self.buffer_dirty {
            return Err("Apply or discard the dirty source buffer before redo".into());
        }
        if self.cursor + 1 == self.history.len() {
            return Err("Nothing to redo".into());
        }
        self.cursor += 1;
        self.next_revision();
        self.last_good = self.history[self.cursor].clone();
        self.draft_seq += 1;
        Ok(self.current())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_is_private_and_invalid_keeps_last_good() {
        let mut w = Workspace::demo().unwrap();
        let head = w.current();
        let source = head.source.replace("E4:1", "D4:1");
        let draft = w.preview_source(&source);
        assert!(draft.valid && draft.dirty);
        assert_eq!(head.revision_id, draft.revision_id);
        assert_eq!(w.current().source, head.source);
        let bad = w.preview_source("!score 1.1 unit=q\n@0\n");
        assert!(!bad.valid);
        assert_eq!(bad.events, draft.events);
        assert!(bad.draft_seq > draft.draft_seq);
        assert_eq!(w.current().source, head.source);
    }
    #[test]
    fn conflict_and_history_are_atomic() {
        let mut w = Workspace::demo().unwrap();
        let a = w.current();
        let b = w
            .apply_source(&a.source.replace("E4:1", "D4:1"), &a.revision_id)
            .unwrap();
        assert_ne!(a.revision_id, b.revision_id);
        assert!(w.apply_source(&a.source, &a.revision_id).is_err());
        let undo = w.undo().unwrap();
        assert_eq!(undo.source, a.source);
        assert_ne!(undo.revision_id, a.revision_id);
        let redo = w.redo().unwrap();
        assert_eq!(redo.source, b.source);
        assert_ne!(redo.revision_id, b.revision_id);
        assert!(w.apply_source("invalid", &redo.revision_id).is_err());
        assert_eq!(w.current().source, redo.source);
    }
    #[test]
    fn applying_after_undo_discards_redo_only_after_validation() {
        let mut w = Workspace::demo().unwrap();
        let a = w.current();
        w.apply_source(&a.source.replace("E4:1", "D4:1"), &a.revision_id)
            .unwrap();
        let u = w.undo().unwrap();
        assert!(w.apply_source("invalid", &u.revision_id).is_err());
        assert!(w.current().can_redo);
        w.apply_source(&a.source.replace("E4:1", "F4:1"), &u.revision_id)
            .unwrap();
        assert!(!w.current().can_redo);
    }
    #[test]
    fn typed_edits_and_dirty_gates() {
        let mut w = Workspace::demo().unwrap();
        let initial = w.current();
        let changed = w
            .set_event_pitch("n1", "F#4", &initial.revision_id)
            .unwrap();
        assert!(
            changed
                .events
                .iter()
                .any(|e| e["id"] == "n1" && e["pitch"] == "F#4")
        );
        assert!(
            changed
                .events
                .iter()
                .all(|e| e["source_line"].as_u64().unwrap_or(0) > 0)
        );
        assert!(w.set_event_pitch("n1", "X4", &changed.revision_id).is_err());
        assert!(
            w.set_event_duration("n1", "1/0", &changed.revision_id)
                .is_err()
        );
        w.preview_source(&format!("{}; unsaved comment\n", changed.source));
        assert!(w.set_event_pitch("n1", "D4", &changed.revision_id).is_err());
        assert!(w.undo().is_err());
        w.preview_source(&changed.source);
        assert!(w.undo().is_ok());
    }
    #[test]
    fn frame_wide_staff_state_does_not_depend_on_line_order() {
        let source = "!score 1.1 unit=q\n!part p instrument=piano staves=2\n!voice v part=p staff=1\n@0\n| p m len=1\nv C4:1 id=n\n= v staff=2\n.\n!end at=1\n";
        let w = Workspace::from_source(source).unwrap();
        assert_eq!(w.current().events[0]["staff"], 2);
    }
}
