//! Closed configuration for a local experimental audition, not product defaults.
use scorestream_app::playback::{self as playback, *};
use scorestream_audio::{assignments::source_sha256, pizzicato::*, *};
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
}
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
pub struct Audition {
    plan: Plan,
    routes: Vec<RouteAssignment>,
    options: RenderOptions,
    source_sha256: String,
}
impl Audition {
    pub fn compile(source: &str, config: Config) -> Result<Self> {
        Self::compile_selected(source, config, None)
    }
    pub fn compile_practice(
        source: &str,
        config: Config,
        _practice: PracticeReviewPolicy,
        midi: scorestream_audio::midi_mixer::LocalMidiPolicy,
    ) -> Result<Self> {
        Self::compile_selected(source, config, Some(midi))
    }
    fn compile_selected(
        source: &str,
        config: Config,
        practice: Option<scorestream_audio::midi_mixer::LocalMidiPolicy>,
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
                scorestream_audio::midi_mixer::prepare_local_midi_mix(
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
                grace_sound: GraceSound::FullGraceThenGatedRemainder,
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
                let g = GraceSoundPolicy::FullGraceThenGatedRemainder;
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
                            Plan::Technique(plan_source_with_practice_joint_policies_and_pizzicato(source,PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,&selected,a,&grace,g,&hold,f,sample_policy)?)
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
        };
        result.preflight()?;
        Ok(result)
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
            json!({"schema":"scorestream.local-audition-preflight.v1","source_sha256":self.source_sha256,"audible_notes":plan.notes.len(),"duration_seconds":plan.duration_seconds,"pizzicato_attacks":techniques,"routes":self.routes,"warnings":plan.warnings,"diagnostics":plan.diagnostics,"local_midi_mix":self.options.local_midi_mix,"native_bank_verified":false,"scope":"pure compiler/route/channel preflight only; render verifies existing bank bytes and actual preset names before PCM"}),
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
            json!({"schema":"scorestream.local-audition-render.v1","source_sha256":self.source_sha256,"output":output,"report":report}),
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
    mix: &scorestream_audio::midi_mixer::LocalMidiMix,
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
