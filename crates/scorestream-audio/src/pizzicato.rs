//! Explicit local TimGM6mb shared-string approximation. No asset is bundled.
use crate::plan::Prepared;
use crate::*;
use scorestream_app::playback::TechniquePlaybackPlan;
use std::collections::BTreeMap;
const SHA: &str = "c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854";
#[derive(Clone, Copy, Debug)]
pub enum PizzicatoSamplePolicy {
    InstalledTimGm6mbSharedStringsV1,
}
#[derive(Clone, Debug, Serialize)]
pub struct TechniqueChannel {
    pub channel: u16,
    pub part: String,
    pub instrument: String,
    pub technique: String,
    pub attacks: usize,
}
#[derive(Debug, Serialize)]
pub struct PizzicatoRenderReport {
    pub render: RenderReport,
    pub technique_channels: Vec<TechniqueChannel>,
    pub approximation: String,
    pub note_dynamics_receipts: Vec<scorestream_app::playback::PracticeNoteDynamicsReceipt>,
}
struct Lanes {
    routes: Vec<RouteAssignment>,
    channels: BTreeMap<String, u16>,
    evidence: Vec<TechniqueChannel>,
    options: RenderOptions,
}
fn invalid(s: &str) -> AudioError {
    AudioError::InvalidPlan(s.into())
}
fn lanes(
    plan: &TechniquePlaybackPlan,
    routes: &[RouteAssignment],
    options: &RenderOptions,
    _policy: PizzicatoSamplePolicy,
) -> Result<Lanes> {
    if !plan.practice_nonrealizations().is_empty() && options.local_midi_mix.is_none() { return Err(invalid("Practice plan requires its matching sealed local MIDI mixer")); }
    let expected = options.bank_verification.as_ref().ok_or_else(|| {
        invalid("Pizzicato requires explicit bank SHA and exact base preset names")
    })?;
    if expected.sha256 != SHA || expected.preset_names.len() != routes.len() {
        return Err(invalid(
            "Pizzicato profile requires the audited local TimGM6mb bytes and exact base route palette",
        ));
    }
    if let Some(mix)=&options.local_midi_mix {
        mix.verify_routes(routes)?;
        if mix.source_sha256()!=plan.source_sha256(){return Err(invalid("Pizzicato compiler witness and MIDI mixer source digests differ"));}
    }
    // Validate the original route identity independently of technique. Native
    // channel overlap checking happens only after the attack-lane assignment.
    let mut result = Lanes {
        routes: routes.to_vec(),
        channels: BTreeMap::new(),
        evidence: Vec::new(),
        options: options.clone(),
    };
    let mut allocated = BTreeMap::new();
    for note in &plan.plan().notes {
        if !plan.pizzicato_attacks().contains(&note.id) {
            continue;
        }
        let identity = scorestream_codec::instruments::resolve_instrument(
            &scorestream_codec::notation::InstrumentId(note.instrument.clone()),
        );
        if !identity.is_some_and(|d| {
            matches!(
                d.id.0.as_str(),
                "violin" | "viola" | "cello" | "double_bass" | "strings"
            )
        }) {
            return Err(invalid(
                "GM pizzicato approximation is only declared for exact catalog orchestral strings and aliases, not arbitrary instruments",
            ));
        }
        if note.midi_note.fract() != 0.0 || !(0.0..=108.0).contains(&note.midi_note) {
            return Err(invalid(
                "Pizzicato attack is outside audited TimGM6mb key zones 0..108",
            ));
        }
        let key = (note.part.clone(), note.instrument.clone());
        let channel = if let Some(channel) = allocated.get(&key) {
            *channel
        } else {
            if !routes
                .iter()
                .any(|r| r.part == note.part && r.instrument == note.instrument)
            {
                return Err(invalid("Missing source route for pizzicato attack"));
            }
            if result.routes.len() >= 256 {
                return Err(invalid("Native MIDI channel representability exceeded"));
            }
            let channel = result.routes.len() as u16;
            result.routes.push(RouteAssignment {
                part: note.part.clone(),
                instrument: note.instrument.clone(),
                bank: 0,
                program: 45,
            });
            result
                .options
                .bank_verification
                .as_mut()
                .unwrap()
                .preset_names
                .push("Pizzicato".into());
            result.evidence.push(TechniqueChannel {
                channel,
                part: note.part.clone(),
                instrument: note.instrument.clone(),
                technique: "authored_pizzicato_gm_shared_strings_approximation".into(),
                attacks: 0,
            });
            allocated.insert(key, channel);
            channel
        };
        result.channels.insert(note.id.clone(), channel);
        result
            .evidence
            .iter_mut()
            .find(|e| e.channel == channel)
            .unwrap()
            .attacks += 1;
    }
    if result.channels.len() != plan.pizzicato_attacks().len() {
        return Err(invalid("Technique attack identity is not an audible note"));
    }
    if let Some(mix)=&options.local_midi_mix {
        result.options.local_midi_mix=Some(mix.with_pizzicato_lanes(plan.source_sha256(),routes,&result.routes[routes.len()..],&expected.sha256)?);
    }
    Ok(result)
}
fn prepared(
    plan: &TechniquePlaybackPlan,
    routes: &[RouteAssignment],
    options: &RenderOptions,
    channels: &BTreeMap<String, u16>,
) -> Result<Prepared> {
    // Expanded palette names belong to Synth, while Prepared validates the
    // unchanged source route set and exact timing before any bank is loaded.
    Prepared::new_with_channels(plan.plan(), routes, options, channels)
}
impl FluidSynthBackend {
    #[allow(clippy::too_many_arguments)]
    pub fn render_pcm_with_pizzicato(
        &self,
        plan: &TechniquePlaybackPlan,
        routes: &[RouteAssignment],
        bank: &Path,
        options: &RenderOptions,
        policy: PizzicatoSamplePolicy,
        cancel: &AtomicBool,
        sink: &mut impl PcmSink,
    ) -> Result<PizzicatoRenderReport> {
        let lanes = lanes(plan, routes, options, policy)?;
        let prepared = prepared(plan, routes, options, &lanes.channels)?;
        let render = self.render_prepared(
            plan.plan(),
            &lanes.routes,
            bank,
            &lanes.options,
            cancel,
            sink,
            prepared,
        )?;
        Ok(report(render, lanes.evidence, plan.note_dynamics_receipts().to_vec()))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn render_wav_with_pizzicato(
        &self,
        plan: &TechniquePlaybackPlan,
        routes: &[RouteAssignment],
        bank: &Path,
        options: &RenderOptions,
        policy: PizzicatoSamplePolicy,
        cancel: &AtomicBool,
        output: &Path,
    ) -> Result<PizzicatoRenderReport> {
        let lanes = lanes(plan, routes, options, policy)?;
        let prepared = prepared(plan, routes, options, &lanes.channels)?;
        let render = crate::wav::render_with(prepared.frames, options, cancel, output, |sink| {
            self.render_prepared(
                plan.plan(),
                &lanes.routes,
                bank,
                &lanes.options,
                cancel,
                sink,
                prepared,
            )
        })?;
        Ok(report(render, lanes.evidence, plan.note_dynamics_receipts().to_vec()))
    }
}
fn report(
    render: RenderReport,
    technique_channels: Vec<TechniqueChannel>,
    note_dynamics_receipts: Vec<scorestream_app::playback::PracticeNoteDynamicsReceipt>,
) -> PizzicatoRenderReport {
    PizzicatoRenderReport {render,technique_channels,note_dynamics_receipts,approximation:"Explicit shared GM Pizzicato preset, three violin source samples; not separately recorded viola/cello/bass or an acoustic model".into()}
}
/// Pure palette/coverage/channel/timing preflight. Bank bytes and actual native
/// preset names are still checked immediately before any PCM is emitted.
pub fn validate_pizzicato_plan(
    plan: &TechniquePlaybackPlan,
    routes: &[RouteAssignment],
    options: &RenderOptions,
    policy: PizzicatoSamplePolicy,
) -> Result<()> {
    let lanes = lanes(plan, routes, options, policy)?;
    prepared(plan, routes, options, &lanes.channels).map(|_| ())
}
