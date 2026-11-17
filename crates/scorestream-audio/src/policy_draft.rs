//! Editable source-bound policy scaffolding, never implicit playback choices.
use crate::audition_session::{self, Audition, Config, PracticeReviewPolicy};
use crate::{assignments::source_sha256, midi_mixer::{self, LocalMidiPolicy}, RouteAssignment};
use serde::Deserialize;
use serde_json::{json, Value};
use scorestream_app::playback::{PracticeTieNoteOnPolicy, PracticeHoldEndpointPolicy, PracticeNoteExpressionPolicy};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SCHEMA: &str = "scorestream.rehearsal-policy-draft.v1";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Draft {
    schema: String,
    source_sha256: String,
    config: Value,
    authored_instruments: Value,
    instructions: Vec<String>,
}
/// Authored identifiers are facts; null bank/program/preset/timing values are
/// intentionally not playable. Even author MIDI program does not select GM bank.
pub fn create(source: &str) -> Result<Value> {
    let doc = scorestream_codec::parse(source).map_err(|e| format!("{e:?}"))?;
    let mut authored = Vec::new();
    for frame in &doc.frames {
        for record in &frame.records {
            if let scorestream_codec::Record::Mark(mark) = record {
                if mark.kind == scorestream_codec::instrument_extension::KIND {
                    authored.push(json!({"part":mark.target,"onset":frame.time,
                        "metadata":scorestream_codec::instrument_extension::decode(mark).map_err(|e|e.to_string())?}));
                }
            }
        }
    }
    let hash = source_sha256(source);
    Ok(json!({"schema":SCHEMA,"source_sha256":hash,"authored_instruments":authored,
        "instructions":[
            "This is an incomplete editable draft, not a playable policy. Replace every null explicitly; do not change either source SHA to fit another score.",
            "Palette part/instrument identifiers are source facts. Select your own bank, zero-based program and exact verified preset name; authored one-based MIDI program does not imply a bank or preset.",
            "After selecting a complete palette, Validate can provide existing practice discovery event identities. It chooses no hold seconds, hold groups or grace allocations.",
            "timing.kind is joint; supported articulation is simple_staccato_accent_v1; grace_sound is full_grace_then_gated_remainder or practice_principal_staccato_accent_in_allocated_remainder_v1; fermata_sound is sustain_selected_anchors. Choose, do not infer.",
            "pizzicato currently requires installed_tim_gm6mb_shared_strings_v1 for this rehearsal, with its shared-sample approximation. final_ending is reject or explicitly continue_to_movement_end; boundary_dynamics is reject or adjacent_opening_visit_attack_percent_v1.",
            "Each hold is {id: your group ID, members: occurrence IDs, added_seconds: exact rational seconds, boundary: exact rational quarter boundary}. Do not group different times by written label.",
            "Each grace is {principal: principal occurrence ID, allocation: {kind: authored_following_percent}} or {principal: principal occurrence ID, allocation: {kind: explicit_following_quarters, members: [{event: grace occurrence ID, quarters: exact rational}]}}. Select one only after reviewing discovery.",
            "Fill holds/graces from source-qualified discovery. Empty arrays are explicit choices and are rejected when required occurrences remain. A complete compile still does not verify a native bank or authorize a WAV."
        ],
        "config":{"source_sha256":hash,"bank_sha256":null,
            "palette":doc.parts.iter().map(|p|json!({"part":p.id,"instrument":p.instrument,"bank":null,"program":null,"preset_name":null})).collect::<Vec<_>>(),
            "timing":{"kind":"joint","articulation":null,"grace_sound":null,"fermata_sound":null,"graces":null,"holds":null},
            "pizzicato":null,"final_ending":null,"boundary_dynamics":null,
            "rendering":{"sample_rate":null,"block_frames":null,"gain":null,"tail_seconds":null,"polyphony":null,"max_notes":null,"max_soundfont_bytes":null}}}))
}
fn config_value(source: &str, value: Value) -> Result<Value> {
    let hash = source_sha256(source);
    let config = if value.get("schema").is_some() {
        let d: Draft = serde_json::from_value(value)?;
        if d.schema != SCHEMA || d.source_sha256 != hash { return Err("Draft schema or source SHA differs from current score".into()); }
        // Editable explanatory fields never grant routing or timing authority.
        let _ = (d.authored_instruments, d.instructions);
        d.config
    } else { value };
    if config.get("source_sha256").and_then(Value::as_str) != Some(hash.as_str()) {
        return Err("Policy source SHA differs from current score".into());
    }
    Ok(config)
}
/// Accept existing Config JSON or a filled draft, without enabling any defaults.
pub fn parse(source: &str, value: Value) -> Result<Config> {
    Ok(serde_json::from_value(config_value(source, value)?)?)
}
fn missing(value: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::Null => out.push(path.into()),
        Value::Object(fields) => for (key, child) in fields {missing(child,&format!("{path}.{key}"),out)},
        Value::Array(items) => for (i, child) in items.iter().enumerate() {missing(child,&format!("{path}[{i}]"),out)},
        _ => (),
    }
}
/// Reuses the real mixer, discovery and practice compiler. Never opens a bank,
/// chooses a duration or creates PCM. Findings are bound to exact current bytes.
pub fn validate(source: &str, value: Value) -> Result<Value> {
    let config = config_value(source, value)?;
    let mut unchosen=Vec::new();missing(&config,"config",&mut unchosen);
    let mut errors=Vec::new();
    let discovery=(|| -> Result<Value> {
        let palette=config.get("palette").and_then(Value::as_array).ok_or("Missing palette")?;
        let routes=palette.iter().map(|p| serde_json::from_value::<RouteAssignment>(json!({
            "part":p["part"],"instrument":p["instrument"],"bank":p["bank"],"program":p["program"]}))).collect::<std::result::Result<Vec<_>,_>>()?;
        let mix=midi_mixer::prepare_local_midi_mix(source,&source_sha256(source),&routes,LocalMidiPolicy::InitialGmCc7CenteredV1)?;
        let ending = serde_json::from_value::<audition_session::FinalEnding>(config.get("final_ending").cloned().unwrap_or(json!("reject")))?;
        audition_session::discover_practice_with_final_ending(source,&mix,ending)
    })();
    let discovery=match discovery {Ok(v)=>Some(v),Err(e)=>{errors.push(format!("Discovery: {e}"));None}};
    let compiled=if unchosen.is_empty() {
        match serde_json::from_value::<Config>(config).map_err(|e|e.into()).and_then(|c|
            Audition::compile_practice(source,PracticeTieNoteOnPolicy::InertWithoutNewNoteOnV1,
                PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1,
                PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1,c,
                PracticeReviewPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,LocalMidiPolicy::InitialGmCc7CenteredV1)) {
            Ok(a)=>Some(a.practice_preflight_report()),Err(e)=>{errors.push(format!("Compile: {e}"));None}
        }
    } else {None};
    Ok(json!({"schema":"scorestream.rehearsal-policy-validation.v1","source_sha256":source_sha256(source),
        "unchosen_fields":unchosen,"errors":errors,"discovery":discovery,
        "policy_compiled":compiled.is_some(),"compiled_report":compiled,
        "native_bank_verified":false,"wav_created":false}))
}
