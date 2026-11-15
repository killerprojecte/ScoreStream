//! Exact source-bound local sample selection and preflight, not a MIDI-intent
//! override or a score importer. Only the existing compiler may create notes.
use crate::{
    AudioError, BankVerification, FluidSynthBackend, PcmSink, RenderOptions, RenderReport, Result,
    RouteAssignment, validate_plan,
};
use scorestream_app::playback::{
    PerformanceDiagnostic, PlaybackPlan, RealizationEffect, plan_source,
};
use scorestream_codec::{Record, instrument_extension, instrument_metadata::InstrumentMetadata};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

pub fn source_sha256(source: &str) -> String {
    hash(source.as_bytes())
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn invalid(message: impl Into<String>) -> AudioError {
    AudioError::InvalidPlan(message.into())
}
fn text(value: &str, name: &str) -> Result<()> {
    if value.is_empty() || value.len() > 4096 || value.contains('\0') {
        return Err(invalid(format!("Invalid {name}")));
    }
    Ok(())
}
fn digest(value: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("Expected a SHA-256 hex digest"));
    }
    Ok(())
}

/// A caller-reviewed profile of an existing authorized local bank, not a download
/// request. Hash/name assertions are enforced by the renderer; musical coverage
/// and license/evidence fields are declared profile facts, not inferred from SF2.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalSampleProfile {
    pub id: String,
    pub bank_path: PathBuf,
    pub bank_sha256: String,
    pub license: String,
    pub provenance_uri: String,
    pub coverage_evidence_sha256: String,
    pub patches: Vec<SamplePatch>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SamplePatch {
    pub id: String,
    pub instrument: String,
    pub bank: u16,
    pub program: u8,
    pub exact_preset_name: String,
    pub regions: Vec<SampleCoverage>,
}
/// Explicit supported key/velocity/note-hold profile. Finite recordings must not be
/// described as infinitely sustaining. Durations include transposition limits
/// chosen in the external reviewed profile; this module does not derive them.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleCoverage {
    pub key_low: u8,
    pub key_high: u8,
    pub velocity_low: u8,
    pub velocity_high: u8,
    pub max_note_seconds: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartSampleSelection {
    pub part: String,
    pub expected_instrument: String,
    /// Authored score-instrument identity, distinct from part ID or patch ID.
    pub expected_source_instrument_id: Option<String>,
    pub patch_id: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct AssignmentDiagnostic {
    pub code: String,
    pub part: Option<String>,
    pub note: Option<String>,
    pub message: String,
    pub blocking: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct NormalizedPartAssignment {
    pub part: String,
    pub instrument: String,
    pub authored_metadata: Option<InstrumentMetadata>,
    pub patch_id: Option<String>,
    pub bank: Option<u16>,
    pub program_zero_based: Option<u8>,
    pub exact_preset_name: Option<String>,
    pub audible_notes_checked: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct AssignmentReport {
    pub schema: String,
    pub exact_sfs_sha256: String,
    pub profile_sha256: String,
    pub profile: LocalSampleProfile,
    pub parts: Vec<NormalizedPartAssignment>,
    pub diagnostics: Vec<AssignmentDiagnostic>,
    pub omitted_diagnostics: usize,
    /// Original structured compiler evidence, including source object/ranges.
    /// Distinct from assignment blockers and never discarded by pure preflight.
    pub compiler_diagnostics: Vec<PerformanceDiagnostic>,
    pub compiler_warnings: Vec<String>,
    pub contract_renderable: bool,
    /// True only after successful rendering. Covers the bank bytes and presets
    /// used by audible routes, not unused profile patches or silent parts.
    pub bank_bytes_and_audible_presets_verified: bool,
}
impl AssignmentReport {
    fn block(
        &mut self,
        code: &str,
        part: Option<&str>,
        note: Option<&str>,
        message: impl Into<String>,
    ) {
        self.contract_renderable = false;
        if self.diagnostics.len() >= 256 {
            self.omitted_diagnostics += 1;
            return;
        }
        self.diagnostics.push(AssignmentDiagnostic {
            code: code.into(),
            part: part.map(Into::into),
            note: note.map(Into::into),
            message: message.into(),
            blocking: true,
        });
    }
}

pub struct AssignmentPreflight {
    report: AssignmentReport,
    qualified: Option<QualifiedSampleRender>,
}
impl AssignmentPreflight {
    pub fn report(&self) -> &AssignmentReport {
        &self.report
    }
    pub fn into_qualified(self) -> Result<QualifiedSampleRender> {
        self.qualified.ok_or_else(|| {
            AudioError::Unsupported(
                "Sample assignment preflight has blocking diagnostics; source was not altered"
                    .into(),
            )
        })
    }
}
/// Opaque, source-qualified request. Callers cannot swap its notes, source digest,
/// assignments, assertions or options after preflight. It is intentionally not
/// deserializable and is not a bypass for compiler restrictions.
pub struct QualifiedSampleRender {
    report: AssignmentReport,
    plan: PlaybackPlan,
    routes: Vec<RouteAssignment>,
    options: RenderOptions,
}
#[derive(Debug, Serialize)]
pub struct AssignedRenderReport {
    pub assignment: AssignmentReport,
    pub audio: RenderReport,
}
impl QualifiedSampleRender {
    pub fn report(&self) -> &AssignmentReport {
        &self.report
    }
    pub fn render_pcm(
        &self,
        backend: &FluidSynthBackend,
        cancel: &AtomicBool,
        sink: &mut impl PcmSink,
    ) -> Result<AssignedRenderReport> {
        let audio = backend.render_pcm(
            &self.plan,
            &self.routes,
            &self.report.profile.bank_path,
            &self.options,
            cancel,
            sink,
        )?;
        Ok(self.completed(audio))
    }
    pub fn render_wav(
        &self,
        backend: &FluidSynthBackend,
        cancel: &AtomicBool,
        output: &Path,
    ) -> Result<AssignedRenderReport> {
        let audio = backend.render_wav(
            &self.plan,
            &self.routes,
            &self.report.profile.bank_path,
            &self.options,
            cancel,
            output,
        )?;
        Ok(self.completed(audio))
    }
    fn completed(&self, audio: RenderReport) -> AssignedRenderReport {
        let mut assignment = self.report.clone();
        assignment.bank_bytes_and_audible_presets_verified = true;
        AssignedRenderReport { assignment, audio }
    }
}

fn validate_profile(profile: &LocalSampleProfile) -> Result<()> {
    for value in [&profile.id, &profile.license, &profile.provenance_uri] {
        text(value, "sample profile text")?;
    }
    digest(&profile.bank_sha256)?;
    digest(&profile.coverage_evidence_sha256)?;
    if !profile.bank_path.is_absolute() || profile.patches.is_empty() || profile.patches.len() > 256
    {
        return Err(invalid(
            "Profile needs an explicit absolute local path and 1..256 patches",
        ));
    }
    let mut ids = BTreeSet::new();
    for patch in &profile.patches {
        for value in [&patch.id, &patch.instrument, &patch.exact_preset_name] {
            text(value, "sample patch text")?;
        }
        if !ids.insert(&patch.id)
            || patch.bank > 16383
            || patch.program > 127
            || patch.regions.is_empty()
            || patch.regions.len() > 256
        {
            return Err(invalid(
                "Duplicate/invalid patch or missing coverage regions",
            ));
        }
        for region in &patch.regions {
            if region.key_low > region.key_high
                || region.key_high > 127
                || region.velocity_low == 0
                || region.velocity_low > region.velocity_high
                || region.velocity_high > 127
                || !region.max_note_seconds.is_finite()
                || region.max_note_seconds <= 0.0
            {
                return Err(invalid("Invalid key/velocity/finite-duration coverage"));
            }
        }
    }
    Ok(())
}

/// One local bank for this backend, explicit per-part patches, and no MIDI intent
/// override. A diagnostic preflight may succeed while qualification is blocked.
/// Hashes bind exact SFS bytes and the normalized JSON serialization of the typed
/// profile. Original MusicXML provenance requires the upstream conversion report
/// and is not claimed here.
pub fn preflight_source(
    source: &str,
    expected_source_sha256: &str,
    profile: &LocalSampleProfile,
    selections: &[PartSampleSelection],
    options: &RenderOptions,
) -> Result<AssignmentPreflight> {
    if source.len() > 4 * 1024 * 1024 || selections.len() > 256 {
        return Err(invalid("Source/selection budget exceeded"));
    }
    digest(expected_source_sha256)?;
    let source_hash = source_sha256(source);
    if !source_hash.eq_ignore_ascii_case(expected_source_sha256) {
        return Err(invalid(
            "Source digest differs from the explicitly selected source revision",
        ));
    }
    validate_profile(profile)?;
    let doc = scorestream_codec::parse(source).map_err(|errors| {
        invalid(
            errors
                .iter()
                .take(8)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        )
    })?;
    let mut metadata = BTreeMap::new();
    for frame in &doc.frames {
        for record in &frame.records {
            if let Record::Mark(mark) = record {
                if mark.kind == instrument_extension::KIND {
                    metadata.insert(
                        mark.target.clone(),
                        instrument_extension::decode(mark).map_err(|e| invalid(e.to_string()))?,
                    );
                }
            }
        }
    }
    let profile_bytes = serde_json::to_vec(profile).map_err(|e| invalid(e.to_string()))?;
    let mut report = AssignmentReport {
        schema: "scorestream.local-sample-assignment.v1".into(),
        exact_sfs_sha256: source_hash,
        profile_sha256: hash(&profile_bytes),
        profile: profile.clone(),
        parts: Vec::new(),
        diagnostics: Vec::new(),
        omitted_diagnostics: 0,
        compiler_diagnostics: Vec::new(),
        compiler_warnings: Vec::new(),
        contract_renderable: true,
        bank_bytes_and_audible_presets_verified: false,
    };
    let mut selected = BTreeMap::new();
    for selection in selections {
        for value in [
            &selection.part,
            &selection.expected_instrument,
            &selection.patch_id,
        ] {
            text(value, "sample selection identity")?;
        }
        if let Some(id) = &selection.expected_source_instrument_id {
            text(id, "expected source identity")?;
        }
        if selected
            .insert(selection.part.as_str(), selection)
            .is_some()
        {
            return Err(invalid("Duplicate part selection"));
        }
        if !doc.parts.iter().any(|part| part.id == selection.part) {
            return Err(invalid("Selection names an undeclared part"));
        }
    }
    let mut patches_by_part = BTreeMap::new();
    for part in &doc.parts {
        let authored = metadata.get(&part.id).cloned();
        let source_id = authored.as_ref().map(|value| value.source_id.clone());
        let patch = if let Some(selection) = selected.get(part.id.as_str()) {
            if selection.expected_instrument != part.instrument
                || selection.expected_source_instrument_id != source_id
            {
                report.block(
                    "selection.source_identity_mismatch",
                    Some(&part.id),
                    None,
                    "Expected source instrument identity differs; no alias/patch override applied",
                );
                None
            } else {
                let patch = profile.patches.iter().find(|p| p.id == selection.patch_id);
                match patch {
                    Some(patch) if patch.instrument == part.instrument => Some(patch),
                    _ => {
                        report.block("selection.patch_unmapped", Some(&part.id), None, "No explicitly selected patch for this exact instrument; no substitute applied");
                        None
                    }
                }
            }
        } else {
            report.block(
                "selection.part_unmapped",
                Some(&part.id),
                None,
                "No explicit local sample assignment for this part",
            );
            None
        };
        if let Some(value) = &authored {
            if value.midi_device.is_some() {
                report.block("source.midi_device_unrealized", Some(&part.id), None, "Authored MIDI device/port intent is retained; this local PCM backend does not send to it");
            }
            if let Some(midi) = &value.midi_instrument {
                report.block("source.midi_assignment_unrealized", Some(&part.id), None, "Authored MIDI channel/program record is retained; a local patch selection does not override its intent");
                if midi.volume.is_some() {
                    report.block("source.volume_unrealized", Some(&part.id), None, "Exact source volume is retained; no CC7 quantization or replacement gain invented");
                }
                if midi.pan.is_some() {
                    report.block("source.pan_unrealized", Some(&part.id), None, "Exact source pan degrees are retained; no stereo/CC10 approximation invented");
                }
            }
        }
        report.parts.push(NormalizedPartAssignment {
            part: part.id.clone(),
            instrument: part.instrument.clone(),
            authored_metadata: authored,
            patch_id: patch.map(|p| p.id.clone()),
            bank: patch.map(|p| p.bank),
            program_zero_based: patch.map(|p| p.program),
            exact_preset_name: patch.map(|p| p.exact_preset_name.clone()),
            audible_notes_checked: 0,
        });
        if let Some(patch) = patch {
            patches_by_part.insert(part.id.as_str(), patch);
        }
    }
    let plan = match plan_source(source) {
        Ok(plan) => Some(plan),
        Err(error) => {
            report.block("source.performance_unavailable", None, None, error);
            None
        }
    };
    let mut routes = Vec::new();
    let mut names = Vec::new();
    if let Some(plan) = &plan {
        report.compiler_diagnostics = plan.diagnostics.clone();
        report.compiler_warnings = plan.warnings.clone();
        for diagnostic in &plan.diagnostics {
            // This entry point accepts no explicit realization policy. Do not
            // turn omissions or approximate/default musical policies into an
            // apparently qualified source. Display-only notation stays visible.
            if diagnostic.effect != RealizationEffect::NotationOnly
                || diagnostic.code == "words_not_realized"
            {
                report.block(
                    "source.realization_unaccepted",
                    None,
                    Some(&diagnostic.object_id),
                    format!(
                        "Compiler diagnostic {} blocks source qualification: {}",
                        diagnostic.code, diagnostic.message
                    ),
                );
            }
        }
        for route in &plan.routes {
            if let Some(patch) = patches_by_part
                .get(route.part.as_str())
                .filter(|p| p.instrument == route.instrument)
            {
                routes.push(RouteAssignment {
                    part: route.part.clone(),
                    instrument: route.instrument.clone(),
                    bank: patch.bank,
                    program: patch.program,
                });
                names.push(patch.exact_preset_name.clone());
            } else {
                report.block(
                    "performance.route_unmapped",
                    Some(&route.part),
                    None,
                    "Audible instrument route has no exact selected patch",
                );
            }
        }
        for note in &plan.notes {
            if let Some(part) = report.parts.iter_mut().find(|part| part.part == note.part) {
                part.audible_notes_checked += 1;
            }
            if let Some(patch) = patches_by_part.get(note.part.as_str()) {
                let covered = note.midi_note.fract() == 0.0
                    && patch.regions.iter().any(|r| {
                        (f64::from(r.key_low)..=f64::from(r.key_high)).contains(&note.midi_note)
                            && (r.velocity_low..=r.velocity_high).contains(&note.velocity)
                            && note.duration_seconds <= r.max_note_seconds
                    });
                if !covered {
                    report.block("sample.coverage_unmapped", Some(&note.part), Some(&note.id),
                    format!("No declared key/velocity/hold coverage for key {}, velocity {}, duration {} seconds", note.midi_note, note.velocity, note.duration_seconds));
                }
                if note.gain_envelope.iter().any(|p| p.gain != 1.0) {
                    report.block("performance.envelope_unrealized", Some(&note.part), Some(&note.id), "Independent fractional note expression is retained and unsupported; no channel-wide approximation");
                }
            }
        }
    } else {
        report.block("sample.coverage_unassessed", None, None, "Performance compilation is blocked; sounding pitches, velocities and hold ranges have not been assessed");
    }
    let mut checked_options = options.clone();
    // This exact profile always supplies the assertions; caller options cannot weaken them.
    checked_options.bank_verification = Some(BankVerification {
        sha256: profile.bank_sha256.clone(),
        preset_names: names,
    });
    if let Some(plan) = &plan {
        if let Err(error) = validate_plan(plan, &routes, &checked_options) {
            report.block(
                "backend.contract_unsupported",
                None,
                None,
                error.to_string(),
            );
        }
    }
    let qualified = if report.contract_renderable {
        Some(QualifiedSampleRender {
            report: report.clone(),
            plan: plan.expect("renderable requires successful compiler"),
            routes,
            options: checked_options,
        })
    } else {
        None
    };
    Ok(AssignmentPreflight { report, qualified })
}
