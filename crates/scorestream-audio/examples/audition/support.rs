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
        if source_sha256(source) != config.source_sha256 {
            return Err("Source bytes differ from the policy receipt; rediscover and explicitly revise the configuration".into());
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
        };
        let routes = config
            .palette
            .iter()
            .map(|p| RouteAssignment {
                part: p.part.clone(),
                instrument: p.instrument.clone(),
                bank: p.bank,
                program: p.program,
            })
            .collect();
        let r = config.rendering;
        let options = RenderOptions {
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
            .find(|d| d.effect == RealizationEffect::Omitted)
        {
            return Err(format!("Audition refuses omitted source sound ({}): {}. Supply a supported explicit interpretation rather than a fallback.",d.code,d.message).into());
        }
        Ok(
            json!({"schema":"scorestream.local-audition-preflight.v1","source_sha256":self.source_sha256,"audible_notes":plan.notes.len(),"duration_seconds":plan.duration_seconds,"pizzicato_attacks":techniques,"routes":self.routes,"warnings":plan.warnings,"diagnostics":plan.diagnostics,"native_bank_verified":false,"scope":"pure compiler/route/channel preflight only; render verifies existing bank bytes and actual preset names before PCM"}),
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
