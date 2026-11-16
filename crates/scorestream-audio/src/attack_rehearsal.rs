//! Closed opt-in rehearsal projection: MIDI attacks only, never faithful dynamics.
use crate::{AudioError, Result, midi_mixer::LocalMidiMix};
use scorestream_app::playback::{
    EnvelopePoint, PlaybackPlan, PracticeNoteDynamicsReceipt, TechniquePlaybackPlan,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RehearsalEnvelopePolicy {
    MidiAttackVelocityOnlyV1,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct EnvelopeOmissionCounts {
    pub affected_notes: usize,
    pub nonunit_points: usize,
    pub nonunit_onset_multipliers: usize,
    pub constant_nonunit_envelopes: usize,
    pub time_varying_envelopes: usize,
    pub equal_offset_steps: usize,
    pub changing_positive_time_segments: usize,
    pub compiler_note_on_receipts: usize,
    pub fractional_note_on_residuals: usize,
    pub note_on_relative_trajectories: usize,
    pub affected_notes_without_note_on_receipt: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct OmittedNoteEnvelope {
    pub event: String,
    pub written_event: Option<String>,
    pub part: String,
    pub midi_attack_velocity: u8,
    pub effective_onset_gain: f64,
    pub original_envelope: Vec<EnvelopePoint>,
}
/// Private construction, no Deserialize. Exact plan fingerprint prevents using a
/// receipt as a generic permission to discard arbitrary envelopes.
#[derive(Clone, Debug, Serialize)]
pub struct AttackOnlyRehearsal {
    source_sha256: String,
    plan_sha256: String,
    policy: RehearsalEnvelopePolicy,
    counts: EnvelopeOmissionCounts,
    omissions: Vec<OmittedNoteEnvelope>,
    note_on_provenance: Vec<PracticeNoteDynamicsReceipt>,
}
impl AttackOnlyRehearsal {
    pub fn counts(&self) -> &EnvelopeOmissionCounts {
        &self.counts
    }
    pub fn omissions(&self) -> &[OmittedNoteEnvelope] {
        &self.omissions
    }
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }
    pub(crate) fn verify(&self, plan: &PlaybackPlan, mix: Option<&LocalMidiMix>) -> Result<()> {
        if mix.is_none_or(|m| m.source_sha256() != self.source_sha256)
            || fingerprint(plan)? != self.plan_sha256
        {
            return Err(invalid(
                "Attack-only rehearsal receipt differs from exact source/mixer/plan",
            ));
        }
        Ok(())
    }
}
fn invalid(s: impl Into<String>) -> AudioError {
    AudioError::InvalidPlan(s.into())
}
fn fingerprint(p: &PlaybackPlan) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(p).map_err(|e| invalid(e.to_string()))?)
    ))
}
fn inspect(plan: &PlaybackPlan) -> Result<(EnvelopeOmissionCounts, Vec<OmittedNoteEnvelope>)> {
    let mut counts = EnvelopeOmissionCounts::default();
    let mut omissions = Vec::new();
    for note in &plan.notes {
        if !note.duration_seconds.is_finite()
            || note.duration_seconds <= 0.0
            || note.velocity == 0
            || note.velocity > 127
        {
            return Err(invalid("Invalid attack/duration for rehearsal"));
        }
        let mut previous = 0.0;
        let mut onset = 1.0;
        for point in &note.gain_envelope {
            if !point.offset_seconds.is_finite()
                || point.offset_seconds < previous
                || point.offset_seconds > note.duration_seconds
                || !point.gain.is_finite()
                || point.gain < 0.0
            {
                return Err(invalid(
                    "Invalid retained envelope; rehearsal cannot discard malformed dynamics",
                ));
            }
            previous = point.offset_seconds;
            if point.offset_seconds == 0.0 {
                onset = point.gain;
            }
        }
        if onset <= 0.0 {
            return Err(invalid(format!(
                "Silent onset on {} cannot become a MIDI attack; later-positive or wholly silent curves are not guessed",
                note.id
            )));
        }
        if note.gain_envelope.iter().all(|p| p.gain == 1.0) {
            continue;
        }
        counts.affected_notes += 1;
        counts.nonunit_points += note.gain_envelope.iter().filter(|p| p.gain != 1.0).count();
        if onset != 1.0 {
            counts.nonunit_onset_multipliers += 1;
        }
        let mut varying = false;
        let (mut at, mut gain) = (0.0, 1.0);
        for (index, point) in note.gain_envelope.iter().enumerate() {
            if index == 0 && point.offset_seconds == 0.0 {
                gain = point.gain;
                continue;
            }
            if point.gain != gain {
                varying = true;
                if point.offset_seconds == at {
                    counts.equal_offset_steps += 1;
                } else {
                    counts.changing_positive_time_segments += 1;
                }
            }
            at = point.offset_seconds;
            gain = point.gain;
        }
        if varying {
            counts.time_varying_envelopes += 1;
        } else {
            counts.constant_nonunit_envelopes += 1;
        }
        omissions.push(OmittedNoteEnvelope {
            event: note.id.clone(),
            written_event: note.written_id.clone(),
            part: note.part.clone(),
            midi_attack_velocity: note.velocity,
            effective_onset_gain: onset,
            original_envelope: note.gain_envelope.clone(),
        });
    }
    Ok((counts, omissions))
}
pub fn prepare_attack_only_rehearsal(
    plan: &TechniquePlaybackPlan,
    mix: &LocalMidiMix,
    policy: RehearsalEnvelopePolicy,
) -> Result<AttackOnlyRehearsal> {
    if plan.practice_nonrealizations().is_empty() || plan.source_sha256() != mix.source_sha256() {
        return Err(invalid(
            "Attack-only projection requires an authenticated practice technique plan and same-source MIDI mixer",
        ));
    }
    let (mut counts, omissions) = inspect(plan.plan())?;
    let receipts = plan.note_dynamics_receipts();
    let mut ids = std::collections::BTreeSet::new();
    for receipt in receipts {
        if !ids.insert(receipt.event.as_str()) {
            return Err(invalid("Duplicate compiler NoteOn receipt"));
        }
        let note = plan
            .plan()
            .notes
            .iter()
            .find(|n| n.id == receipt.event)
            .ok_or_else(|| invalid("Compiler NoteOn receipt has no attack"))?;
        if receipt.integer_velocity != note.velocity
            || receipt.realized_envelope != note.gain_envelope
        {
            return Err(invalid(
                "Compiler NoteOn provenance differs from realized plan",
            ));
        }
        counts.compiler_note_on_receipts += 1;
        if receipt.exact_note_on
            != scorestream_codec::Rational::from_integer(i64::from(receipt.integer_velocity))
        {
            counts.fractional_note_on_residuals += 1;
        }
        if receipt.relative_trajectory_applied {
            counts.note_on_relative_trajectories += 1;
        }
    }
    counts.affected_notes_without_note_on_receipt = omissions
        .iter()
        .filter(|n| !ids.contains(n.event.as_str()))
        .count();
    Ok(AttackOnlyRehearsal {
        source_sha256: plan.source_sha256().into(),
        plan_sha256: fingerprint(plan.plan())?,
        policy,
        counts,
        omissions,
        note_on_provenance: receipts.to_vec(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use scorestream_app::playback::plan_source;
    fn plan() -> PlaybackPlan {
        plan_source("!score 1.1 unit=q\n!part p instrument=flute staves=1\n!voice v part=p staff=1\n@0\n| p m len=4\nv C4:4 id=a\n.\n!end at=4\n").unwrap()
    }
    #[test]
    fn refuses_silent_later_positive_invalid_and_nonfinite_curves() {
        for points in [
            vec![(0.0, 0.0), (0.5, 1.0)],
            vec![(0.0, 0.0)],
            vec![(0.0, -1.0)],
            vec![(0.0, f64::NAN)],
            vec![(0.5, 1.0), (0.1, 0.5)],
            vec![(4.0, 1.1)],
        ] {
            let mut p = plan();
            p.notes[0].gain_envelope = points
                .into_iter()
                .map(|(offset_seconds, gain)| EnvelopePoint {
                    offset_seconds,
                    gain,
                })
                .collect();
            assert!(inspect(&p).is_err());
        }
    }
    #[test]
    fn counts_distinct_omission_shapes_without_guessing_provenance() {
        let mut p = plan();
        p.notes[0].gain_envelope = vec![
            EnvelopePoint {
                offset_seconds: 0.0,
                gain: 1.001,
            },
            EnvelopePoint {
                offset_seconds: 0.5,
                gain: 1.001,
            },
        ];
        let (c, _) = inspect(&p).unwrap();
        assert_eq!(c.constant_nonunit_envelopes, 1);
        assert_eq!(c.nonunit_onset_multipliers, 1);
        p.notes[0].gain_envelope.extend([
            EnvelopePoint {
                offset_seconds: 1.0,
                gain: 2.0,
            },
            EnvelopePoint {
                offset_seconds: 1.0,
                gain: 0.5,
            },
        ]);
        let (c, _) = inspect(&p).unwrap();
        assert_eq!(c.time_varying_envelopes, 1);
        assert_eq!(c.equal_offset_steps, 1);
        assert_eq!(c.changing_positive_time_segments, 1);
    }
}
