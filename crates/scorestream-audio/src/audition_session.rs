//! Explicit Linux audition session shared by CLI and future desktop adapters.
//! Captures source bytes and compiled policy/palette witnesses; never changes
//! default playback. Callers own chooser/window authority and cancellation.
use scorestream_app::playback::{self as playback, *};
use crate::{assignments::source_sha256, pizzicato::*, *};
use scorestream_codec::{Rational, fermata_timing::*, notation::*};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::Path, sync::atomic::AtomicBool};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub source_sha256: String,
    pub bank_sha256: String,
    pub palette: Vec<Patch>,
    pub timing: Timing,
    pub pizzicato: Pizzicato,
    pub rendering: Rendering,
    #[serde(default)]
    pub final_ending:FinalEnding,
    #[serde(default)]
    pub boundary_dynamics:BoundaryDynamics,
}
#[derive(Clone,Copy,Default,Deserialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub enum BoundaryDynamics { #[default] Reject, AdjacentOpeningVisitAttackPercentV1 }
#[derive(Clone,Copy,Default,Deserialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub enum FinalEnding { #[default] Reject, ContinueToMovementEnd }
impl FinalEnding {fn core(self)->scorestream_codec::repeat_traversal::UnclosedFinalEndingPolicy {match self {Self::Reject=>scorestream_codec::repeat_traversal::UnclosedFinalEndingPolicy::Reject,Self::ContinueToMovementEnd=>scorestream_codec::repeat_traversal::UnclosedFinalEndingPolicy::ContinueToMovementEnd}}}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    part: String,
    instrument: String,
    bank: u16,
    program: u8,
    preset_name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pizzicato {
    RejectUnresolved,
    InstalledTimGm6mbSharedStringsV1,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Articulation {
    SimpleStaccatoAccentV1,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraceSound {
    FullGraceThenGatedRemainder,
    PracticePrincipalStaccatoAccentInAllocatedRemainderV1,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FermataSound {
    SustainSelectedAnchors,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Timing {
    Ordinary,
    Joint {
        articulation: Articulation,
        grace_sound: GraceSound,
        fermata_sound: FermataSound,
        graces: Vec<Grace>,
        holds: Vec<Hold>,
    },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grace {
    principal: EventId,
    allocation: Allocation,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Allocation {
    AuthoredFollowingPercent,
    ExplicitFollowingQuarters { members: Vec<Member> },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    event: EventId,
    quarters: Rational,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hold {
    id: String,
    members: Vec<EventId>,
    added_seconds: Rational,
    boundary: Rational,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rendering {
    sample_rate: u32,
    block_frames: usize,
    gain: f64,
    tail_seconds: f64,
    polyphony: u32,
    max_notes: usize,
    max_soundfont_bytes: u64,
}
enum Plan {
    Ordinary(PlaybackPlan),
    Technique(TechniquePlaybackPlan),
}
/// Native readiness retained in the host, not round-tripped through a WebView.
/// No Deserialize or public constructor can mint a prepared session from JSON.
pub struct DryReadiness {
    source_sha256: String,
    bank: std::path::PathBuf,
    report: Value,
}
impl DryReadiness {
    pub fn report(&self) -> &Value { &self.report }
    pub fn source_sha256(&self) -> &str { &self.source_sha256 }
}
pub struct Audition {
    plan: Plan,
    routes: Vec<RouteAssignment>,
    options: RenderOptions,
    source_sha256: String,
    captured_source: String,
}
impl Audition {
    /// Identifier of the captured source, not a claim that an editor is unchanged.
    pub fn source_sha256(&self) -> &str { &self.source_sha256 }
    pub fn verify_snapshot(&self, current_source: &str) -> Result<()> {
        if self.captured_source != current_source {
            return Err("Source snapshot changed; explicitly prepare a new audition session".into());
        }
        Ok(())
    }
    fn check_request(&self, source: &str, cancel: &AtomicBool) -> Result<()> {
        self.verify_snapshot(source)?;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {return Err(AudioError::Cancelled.into());}
        Ok(())
    }
    /// A desktop worker may call this with its window-owned cancel token. It
    /// never chooses a bank/path or starts default playback on the caller's behalf.
    pub fn prepare_dry_snapshot(&self, current_source: &str, bank: &Path, cancel: &AtomicBool) -> Result<DryReadiness> {
        self.check_request(current_source,cancel)?;
        let Plan::Technique(plan)=&self.plan else {return Err("Dry probe requires opaque technique plan".into());};
        let bank=bank.canonicalize()?;
        let report=crate::dry_admission::probe_until_idle(&self.captured_source,plan,&self.routes,&bank,&self.options,crate::dry_lanes::DryTailPolicy::HoldLastGainAtNoteOffV1,crate::pizzicato::PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,cancel)?;
        // Match the exact serializer/parser comparison contract of the existing
        // native atomic replay; callers cannot mutate this retained observation.
        let report=serde_json::from_slice(&serde_json::to_vec(&report)?)?;
        Ok(DryReadiness { source_sha256:self.source_sha256.clone(), bank, report })
    }
    pub fn render_prepared_snapshot(&self, current_source: &str, prepared: &DryReadiness, output: &Path, cancel: &AtomicBool) -> Result<Value> {
        self.check_request(current_source,cancel)?;
        if prepared.source_sha256!=self.source_sha256 {return Err("Readiness belongs to a different source snapshot".into());}
        self.render_dry_snapshot(current_source,&prepared.bank,&prepared.report,output,cancel)
    }
    /// Rechecks the current source before opening any output. The native engine
    /// freshly verifies readiness and publishes atomically only on complete match.
    pub fn render_dry_snapshot(&self, current_source: &str, bank: &Path, expected: &Value, output: &Path, cancel: &AtomicBool) -> Result<Value> {
        self.check_request(current_source,cancel)?;
        let Plan::Technique(plan)=&self.plan else {return Err("Dry render requires opaque technique plan".into());};
        Ok(serde_json::to_value(crate::dry_admission::render_verified_wav(&self.captured_source,plan,&self.routes,bank,&self.options,crate::dry_lanes::DryTailPolicy::HoldLastGainAtNoteOffV1,crate::pizzicato::PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,cancel,expected,output)?)?)
    }
    pub fn dry_render_verified(&self, source: &str, bank: &Path, expected: &Value, output: &Path) -> Result<Value> {
        self.render_dry_snapshot(source,bank,expected,output,&AtomicBool::new(false))
    }
    pub fn dry_admission_until_idle(&self, source: &str, bank: &Path) -> Result<Value> {
        let Plan::Technique(plan)=&self.plan else {return Err("Dry probe requires opaque technique plan".into());};
        self.verify_snapshot(source)?;
        Ok(serde_json::to_value(crate::dry_admission::probe_until_idle(&self.captured_source,plan,&self.routes,bank,&self.options,crate::dry_lanes::DryTailPolicy::HoldLastGainAtNoteOffV1,crate::pizzicato::PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,&AtomicBool::new(false))?)?)
    }
    pub fn dry_admission_probe(&self, source: &str, bank: &Path) -> Result<Value> {
        self.verify_snapshot(source)?;
        let Plan::Technique(plan)=&self.plan else {return Err("Dry probe requires opaque technique plan".into());};
        Ok(serde_json::to_value(crate::dry_admission::probe(&self.captured_source,plan,&self.routes,bank,&self.options,crate::dry_lanes::DryTailPolicy::HoldLastGainAtNoteOffV1,crate::pizzicato::PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,&AtomicBool::new(false))?)?)
    }
    pub fn compile(source: &str, config: Config) -> Result<Self> {
        Self::compile_selected(source, config, None, None, None, None)
    }
    pub fn compile_practice(
        source: &str,
        tie_note_on: PracticeTieNoteOnPolicy,
        endpoint: PracticeHoldEndpointPolicy,
        note_expression: PracticeNoteExpressionPolicy,
        config: Config,
        _practice: PracticeReviewPolicy,
        midi: crate::midi_mixer::LocalMidiPolicy,
    ) -> Result<Self> {
        Self::compile_selected(
            source,
            config,
            Some(midi),
            Some(tie_note_on),
            Some(endpoint),
            Some(note_expression),
        )
    }
    fn compile_selected(
        source: &str,
        config: Config,
        practice: Option<crate::midi_mixer::LocalMidiPolicy>,
        tie_note_on: Option<PracticeTieNoteOnPolicy>,
        endpoint: Option<PracticeHoldEndpointPolicy>,
        note_expression: Option<PracticeNoteExpressionPolicy>,
    ) -> Result<Self> {
        if source_sha256(source) != config.source_sha256 {
            return Err("Source bytes differ from the policy receipt; rediscover and explicitly revise the configuration".into());
        }
        let routes: Vec<RouteAssignment> = config
            .palette
            .iter()
            .map(|p| RouteAssignment {
                part: p.part.clone(),
                instrument: p.instrument.clone(),
                bank: p.bank,
                program: p.program,
            })
            .collect();

        if practice.is_some()
            && (!matches!(&config.timing, Timing::Joint { .. })
                || !matches!(
                    &config.pizzicato,
                    Pizzicato::InstalledTimGm6mbSharedStringsV1
                ))
        {
            return Err(
                "Practice requires explicit joint policies and installed sample approximation"
                    .into(),
            );
        }
        let mix = practice
            .map(|p| {
                crate::midi_mixer::prepare_local_midi_mix(
                    source,
                    &config.source_sha256,
                    &routes,
                    p,
                )
            })
            .transpose()?;
        if let Some(m) = &mix {
            m.verify_source(source)?;
        }
        if config.boundary_dynamics!=BoundaryDynamics::Reject && (mix.is_none() || !matches!(&config.timing,Timing::Joint {..}) || !matches!(&config.pizzicato,Pizzicato::InstalledTimGm6mbSharedStringsV1) || config.final_ending!=FinalEnding::Reject) {return Err("Boundary dynamics requires authenticated practice joint path without an additional final-ending policy".into());}
        if config.final_ending!=FinalEnding::Reject && (mix.is_none() || !matches!(&config.timing,Timing::Joint {..}) || !matches!(&config.pizzicato,Pizzicato::InstalledTimGm6mbSharedStringsV1)) {return Err("Explicit final ending requires authenticated practice joint path".into());}
        let sample_policy = PizzicatoSoundPolicy::GeneralMidiSharedStringsApproximation;
        let plan = match config.timing {
            Timing::Ordinary => match config.pizzicato {
                Pizzicato::RejectUnresolved => Plan::Ordinary(playback::plan_source(source)?),
                Pizzicato::InstalledTimGm6mbSharedStringsV1 => {
                    Plan::Technique(plan_source_with_pizzicato(source, sample_policy)?)
                }
            },
            Timing::Joint {
                articulation: Articulation::SimpleStaccatoAccentV1,
                grace_sound,
                fermata_sound: FermataSound::SustainSelectedAnchors,
                graces,
                holds,
            } => {
                let grace = ExplicitGracePolicy {
                    groups: graces
                        .into_iter()
                        .map(|g| ExplicitGraceGroup {
                            principal: g.principal,
                            allocation: match g.allocation {
                                Allocation::AuthoredFollowingPercent => {
                                    GraceAllocation::AuthoredFollowingPercent
                                }
                                Allocation::ExplicitFollowingQuarters { members } => {
                                    GraceAllocation::ExplicitFollowingQuarters {
                                        members: members
                                            .into_iter()
                                            .map(|m| GraceQuarterAllocation {
                                                event: m.event,
                                                quarters: m.quarters,
                                            })
                                            .collect(),
                                    }
                                }
                            },
                        })
                        .collect(),
                };
                let hold = ExplicitSynchronizedFermataPolicy {
                    groups: holds
                        .into_iter()
                        .map(|h| ExplicitSynchronizedHoldGroup {
                            boundary: h.boundary,
                            hold: ExplicitHoldGroup {
                                id: h.id,
                                members: h.members,
                                added_seconds: h.added_seconds,
                            },
                        })
                        .collect(),
                };
                let a = ExplicitArticulationPolicy::SimpleStaccatoAccentV1;
                let g = match grace_sound {GraceSound::FullGraceThenGatedRemainder=>GraceSoundPolicy::FullGraceThenGatedRemainder,GraceSound::PracticePrincipalStaccatoAccentInAllocatedRemainderV1=>GraceSoundPolicy::PracticePrincipalStaccatoAccentInAllocatedRemainderV1};
                let f = FermataSoundPolicy::SustainSelectedAnchors;
                match config.pizzicato {
                    Pizzicato::RejectUnresolved => Plan::Ordinary(
                        plan_source_with_explicit_joint_policies(source, a, &grace, g, &hold, f)?
                            .plan()
                            .clone(),
                    ),
                    Pizzicato::InstalledTimGm6mbSharedStringsV1 => {
                        if let Some(mix) = &mix {
                            let selected = mix
                                .routes()
                                .iter()
                                .map(|r| PracticeMidiSelection {
                                    part: r.route.part.clone(),
                                    instrument: r.route.instrument.clone(),
                                    bank: r.route.bank,
                                    program_zero_based: r.route.program,
                                })
                                .collect::<Vec<_>>();
                            if config.boundary_dynamics==BoundaryDynamics::AdjacentOpeningVisitAttackPercentV1 {
                                Plan::Technique(plan_source_with_boundary_practice_joint_policies_and_pizzicato(source,PracticeBoundaryAttackPolicy::AdjacentOpeningVisitAttackPercentV1,tie_note_on.ok_or("Practice requires explicit tied NoteOn policy")?,endpoint.ok_or("Practice requires explicit endpoint policy")?,note_expression.ok_or("Practice requires explicit note expression policy")?,PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,&selected,a,&grace,g,&hold,f,sample_policy)?)
                            } else {
                            Plan::Technique(plan_source_with_practice_joint_policies_pizzicato_and_final_ending(source,tie_note_on.ok_or("Practice requires explicit tied NoteOn policy")?,endpoint.ok_or("Practice requires explicit endpoint policy")?,note_expression.ok_or("Practice requires explicit note expression policy")?,PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,&selected,a,&grace,g,&hold,f,sample_policy,config.final_ending.core())?)
                            }
                        } else {
                            Plan::Technique(plan_source_with_joint_policies_and_pizzicato(
                                source,
                                a,
                                &grace,
                                g,
                                &hold,
                                f,
                                sample_policy,
                            )?)
                        }
                    }
                }
            }
        };
        let r = config.rendering;
        let options = RenderOptions {
            local_midi_mix: mix,
            attack_only_rehearsal: None,
            sample_rate: r.sample_rate,
            block_frames: r.block_frames,
            gain: r.gain,
            tail_seconds: r.tail_seconds,
            polyphony: r.polyphony,
            max_notes: r.max_notes,
            max_soundfont_bytes: r.max_soundfont_bytes,
            bank_verification: Some(BankVerification {
                sha256: config.bank_sha256,
                preset_names: config.palette.into_iter().map(|p| p.preset_name).collect(),
            }),
        };
        let result = Self {
            plan,
            routes,
            options,
            source_sha256: config.source_sha256,
            captured_source: source.to_owned(),
        };
        if practice.is_none() {
            result.preflight()?;
        }
        Ok(result)
    }
    /// Explicit listening approximation, never selected by compile_practice itself.
    pub fn with_attack_only_rehearsal(mut self, policy:crate::attack_rehearsal::RehearsalEnvelopePolicy)->Result<Self> {
        let Plan::Technique(plan)=&self.plan else {return Err("Attack-only rehearsal requires authenticated practice technique plan".into());};
        let mix=self.options.local_midi_mix.as_ref().ok_or("Missing same-source MIDI mixer")?;
        self.options.attack_only_rehearsal=Some(crate::attack_rehearsal::prepare_attack_only_rehearsal(plan,mix,policy)?);
        Ok(self)
    }
    /// Planned event boundary, not a claim about native sample release completion.
    pub fn planned_endpoints(&self) -> Value {
        let plan = match &self.plan { Plan::Ordinary(p) => p, Plan::Technique(p) => p.plan() };
        let last = plan.notes.iter().max_by(|a,b| (a.start_seconds+a.duration_seconds).total_cmp(&(b.start_seconds+b.duration_seconds)));
        json!({"last_note_off":last.map(|n|json!({"event":n.id,"written_event":n.written_id,"part":n.part,"seconds":n.start_seconds+n.duration_seconds,"scheduled_frame":((n.start_seconds+n.duration_seconds).min(plan.duration_seconds)*f64::from(self.options.sample_rate)+0.5).floor() as u64})),"score_seconds":plan.duration_seconds,"selected_tail_seconds":self.options.tail_seconds,"sample_rate":self.options.sample_rate,"scope":"Planned NoteOff frame uses native adapter rounding; native response is block-quantized. Fixed tail is not proof of complete patch decay."})
    }
    pub fn practice_preflight_report(&self) -> Value {
        let Plan::Technique(p) = &self.plan else {
            return json!({"compiled":false,"error":"Not a technique plan"});
        };
        let native = self.preflight();
        json!({"schema":"scorestream.practice-plan-preflight.v1","source_sha256":self.source_sha256,"compiled":true,"planned_endpoints":self.planned_endpoints(),"native_preflight_passed":native.is_ok(),"native_preflight_error":native.err().map(|e|e.to_string()),"audible_notes":p.plan().notes.len(),"duration_seconds":p.plan().duration_seconds,"pizzicato_attacks":p.pizzicato_attacks().len(),"warnings":p.plan().warnings,"diagnostics":p.plan().diagnostics,"note_dynamics_receipts":p.note_dynamics_receipts(),"final_ending_receipt":p.final_ending_receipt(),"boundary_dynamics_receipt":p.boundary_dynamics_receipt(),"local_midi_mix":self.options.local_midi_mix,"attack_only_rehearsal":self.options.attack_only_rehearsal,"native_bank_verified":false})
    }
    pub fn preflight(&self) -> Result<Value> {
        let (plan, techniques) = match &self.plan {
            Plan::Ordinary(p) => {
                validate_plan(p, &self.routes, &self.options)?;
                (p, 0)
            }
            Plan::Technique(p) => {
                validate_pizzicato_plan(
                    p,
                    &self.routes,
                    &self.options,
                    PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
                )?;
                (p.plan(), p.pizzicato_attacks().len())
            }
        };
        if let Some(d) = plan
            .diagnostics
            .iter()
            .find(|d| d.effect == RealizationEffect::Omitted && !(self.options.local_midi_mix.is_some() && matches!(&self.plan, Plan::Technique(p) if !p.practice_nonrealizations().is_empty()) && matches!(d.code.as_str(),"practice.wedge_unrepresented"|"practice.dashes_unrepresented"|"practice.ornaments_unrepresented"|"practice.slur_unrepresented")))
        {
            return Err(format!("Audition refuses omitted source sound ({}): {}. Supply a supported explicit interpretation rather than a fallback.",d.code,d.message).into());
        }
        Ok(
            json!({"schema":"scorestream.local-audition-preflight.v1","source_sha256":self.source_sha256,"audible_notes":plan.notes.len(),"duration_seconds":plan.duration_seconds,"pizzicato_attacks":techniques,"routes":self.routes,"warnings":plan.warnings,"diagnostics":plan.diagnostics,"local_midi_mix":self.options.local_midi_mix,"attack_only_rehearsal":self.options.attack_only_rehearsal,"native_bank_verified":false,"scope":"pure compiler/route/channel preflight only; render verifies existing bank bytes and actual preset names before PCM"}),
        )
    }
    pub fn render(&self, bank: &Path, output: &Path) -> Result<Value> {
        let backend = FluidSynthBackend::open_installed()?;
        let cancel = AtomicBool::new(false);
        let report = match &self.plan {
            Plan::Ordinary(p) => serde_json::to_value(backend.render_wav(
                p,
                &self.routes,
                bank,
                &self.options,
                &cancel,
                output,
            )?)?,
            Plan::Technique(p) => serde_json::to_value(backend.render_wav_with_pizzicato(
                p,
                &self.routes,
                bank,
                &self.options,
                PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
                &cancel,
                output,
            )?)?,
        };
        Ok(
            json!({"schema":"scorestream.local-audition-render.v1","source_sha256":self.source_sha256,"output":output,"planned_endpoints":self.planned_endpoints(),"report":report}),
        )
    }
}
/// Discovery supplies identities/coordinates only, never suggested hold durations.
pub fn discover(source: &str) -> Result<Value> {
    let d = discover_source_joint_performance(
        source,
        ExplicitArticulationPolicy::SimpleStaccatoAccentV1,
    )?;
    Ok(
        json!({"source_sha256":source_sha256(source),"selected_articulation_policy":"simple_staccato_accent_v1","graces":d.graces.occurrences.iter().map(|g|json!({"event":g.id,"written_event":g.written_id,"principal":g.principal,"part":g.part,"order":g.order,"principal_start":g.principal_start,"principal_end":g.principal_end,"authored_timing":g.timing})).collect::<Vec<_>>(),"fermatas":d.fermatas.occurrences.iter().map(|f|json!({"event":f.id,"written_event":f.written_id,"part":f.part,"start":f.quarter_start,"end":f.quarter_end})).collect::<Vec<_>>(),"policy":"No duration, grouping or sample assignment has been chosen."}),
    )
}

/// Non-playable trace; it deliberately does not call the sound-qualified discovery API.
pub fn trace(source: &str) -> Result<Value> {
    let doc = scorestream_codec::parse(source).map_err(|e| format!("{e:?}"))?;
    let trace =
        scorestream_codec::notation::trace_document_route(&doc).map_err(|e| format!("{e:?}"))?;
    Ok(json!({"source_sha256":source_sha256(source),"trace":trace}))
}

/// A closed, explicit review request. This is not permission to render: no
/// compiler profile accepts it yet, and it cannot construct a playback plan.
#[derive(Clone, Copy, Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticeReviewPolicy {
    WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
}
/// Durable source-bound readiness evidence. All typed source records remain
/// unchanged. Missing acoustic behavior is named even when no plan can be made.
pub fn review_practice(source: &str, policy: PracticeReviewPolicy) -> Result<Value> {
    use scorestream_codec::{Record, instrument_extension};
    let doc = scorestream_codec::parse(source).map_err(|e| format!("{e:?}"))?;
    let route = trace_document_route(&doc).map_err(|e| format!("{e:?}"))?;
    let descriptions = [
        (
            "scorestream.wedge@1",
            "No crescendo/diminuendo gain trajectory is realized from authored wedges.",
        ),
        (
            "scorestream.dashes@1",
            "No acoustic continuation or termination is inferred from authored dashed direction spans.",
        ),
        (
            "scorestream.ornaments@1",
            "No ornamental auxiliary attacks, tremolo repetitions, pitch alteration or timing are added.",
        ),
        (
            "scorestream.slur@1",
            "No legato overlap, phrasing, bowing or articulation change is inferred from authored slurs.",
        ),
    ];
    let disclosures: Vec<_> = descriptions
        .into_iter()
        .map(|(family, behavior)| {
            json!({
                "family":family,"present":doc.required_extensions.iter().any(|e|e==family),
                "unrepresented_behavior":behavior,"source_records_retained":true
            })
        })
        .collect();
    let mut metadata = Vec::new();
    for record in doc.frames.iter().flat_map(|f| &f.records) {
        if let Record::Mark(mark) = record {
            if mark.kind == instrument_extension::KIND {
                let authored = instrument_extension::decode(mark).map_err(|e| e.to_string())?;
                metadata.push(json!({"part":mark.target,"authored":authored}));
            }
        }
    }
    let midi_blocker = instrument_extension::has_midi_intent(&doc);
    let strict_result = discover_document_joint_performance(
        &doc,
        ExplicitArticulationPolicy::SimpleStaccatoAccentV1,
    );
    let strict_errors = match strict_result {
        Ok(_) => Vec::new(),
        Err(errors) => errors.into_iter().map(|e| e.to_string()).collect(),
    };
    Ok(json!({
        "schema":"scorestream.practice-review.v1","source_sha256":source_sha256(source),
        "selected_review_policy":policy,"disclosures":disclosures,"authored_instruments":metadata,
        "route":route,"strict_joint_discovery_errors":strict_errors,
        "independently_verified_midi_route_blocker":midi_blocker,
        "midi_route_blocker_code":if midi_blocker {Some("instrument.performance_unsupported")} else {None},
        "midi_scope":"Source channel and port are external addresses, not local synth channel allocations. Programs require explicit bank/preset selection; exact volume percent and pan degrees have no selected realization. No rounding, gain substitution or external MIDI delivery occurred.",
        "playback_plan_created":false,"contract_renderable":false,
        "remaining_requirements":["A separate sealed practice compiler and renderer receipt must explicitly carry the four nonrealizations.","Resolve authored instrument routing and mixer semantics without discarding them.","Choose and validate explicit source-qualified grace/fermata policies; no hold duration is selected by this review.","Validate real note/tie/clock semantics and native palette, overlap, envelope and tail preflight before rendering."]
    }))
}

/// Native-backed practice discovery: verify the sealed MIDI receipt against the
/// exact source before permitting the narrowly scoped core discovery profile.
pub fn discover_practice(
    source: &str,
    mix: &crate::midi_mixer::LocalMidiMix,
) -> Result<Value> {
    mix.verify_source(source)?;
    let doc = scorestream_codec::parse(source).map_err(|e| format!("{e:?}"))?;
    let routes = mix
        .routes()
        .iter()
        .map(|r| PracticeMidiSelection {
            part: r.route.part.clone(),
            instrument: r.route.instrument.clone(),
            bank: r.route.bank,
            program_zero_based: r.route.program,
        })
        .collect::<Vec<_>>();
    let result = discover_document_practice_joint(
        &doc,
        PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
        ExplicitArticulationPolicy::SimpleStaccatoAccentV1,
        &routes,
    );
    match result {
        Ok(q) => Ok(
            json!({"source_sha256":source_sha256(source),"midi_receipt":mix,"nonrealizations":q.nonrealizations(),"playback_plan_created":false,"hold_seconds_selected":false,"grace_occurrences":q.discovery().graces.occurrences.len(),"fermatas":q.discovery().fermatas.occurrences.iter().map(|f|json!({"event":f.id,"written_event":f.written_id,"part":f.part,"start":f.quarter_start,"end":f.quarter_end})).collect::<Vec<_>>()}),
        ),
        Err(e) => Ok(
            json!({"source_sha256":source_sha256(source),"midi_receipt":mix,"playback_plan_created":false,"hold_seconds_selected":false,"practice_discovery_errors":e.iter().map(ToString::to_string).collect::<Vec<_>>()}),
        ),
    }
}
