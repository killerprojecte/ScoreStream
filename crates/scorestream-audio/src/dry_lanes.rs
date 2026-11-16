//! Experimental dry PCM building blocks, not wired to any renderer.
//! Backend must provide exclusive, non-aliased dry groups and authoritative
//! playing-voice observations at completed synthesis boundaries. No tail timeout.
use crate::{AudioError, Result};
use scorestream_app::playback::{EnvelopePoint, TechniquePlaybackPlan};
use serde::Serialize;
use sha2::{Digest, Sha256};
fn invalid(s: &str) -> AudioError {
    AudioError::InvalidPlan(s.into())
}
#[derive(Clone, Copy, Debug, Serialize)]
pub enum DryTailPolicy {
    HoldLastGainAtNoteOffV1,
}
#[derive(Clone, Debug, Serialize)]
pub struct DryNoteReceipt {
    pub event: String,
    pub start_frame: u64,
    pub noteoff_frame: u64,
    pub duration_seconds: f64,
    pub original_envelope: Vec<EnvelopePoint>,
}
/// Immutable source/plan-bound recipe. Not a fidelity or backend authorization.
#[derive(Clone, Debug, Serialize)]
pub struct DryEnvelopeSchedule {
    source_sha256: String,
    plan_sha256: String,
    sample_rate: u32,
    tail_policy: DryTailPolicy,
    notes: Vec<DryNoteReceipt>,
}
impl DryEnvelopeSchedule {
    pub fn prepare(
        plan: &TechniquePlaybackPlan,
        sample_rate: u32,
        tail_policy: DryTailPolicy,
    ) -> Result<Self> {
        if sample_rate == 0 {
            return Err(invalid("Zero sample rate"));
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut notes = Vec::new();
        for n in &plan.plan().notes {
            if !ids.insert(&n.id) {
                return Err(invalid("Duplicate performed identity"));
            }
            if !n.start_seconds.is_finite() || n.start_seconds < 0.0 {
                return Err(invalid("Invalid note onset"));
            }
            validate_curve(n.duration_seconds, &n.gain_envelope)?;
            let start_frame = crate::plan::frame(n.start_seconds, sample_rate)?;
            let noteoff_frame =
                crate::plan::frame(n.start_seconds + n.duration_seconds, sample_rate)?;
            if noteoff_frame <= start_frame {
                return Err(invalid("Note collapses at selected sample rate"));
            }
            notes.push(DryNoteReceipt {
                event: n.id.clone(),
                start_frame,
                noteoff_frame,
                duration_seconds: n.duration_seconds,
                original_envelope: n.gain_envelope.clone(),
            });
        }
        Ok(Self {
            source_sha256: plan.source_sha256().into(),
            plan_sha256: format!(
                "{:x}",
                Sha256::digest(
                    serde_json::to_vec(plan.plan())
                        .map_err(|_| invalid("Plan serialization failed"))?
                )
            ),
            sample_rate,
            tail_policy,
            notes,
        })
    }
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }
    pub fn plan_sha256(&self) -> &str {
        &self.plan_sha256
    }
    pub fn verify_plan(&self, plan: &TechniquePlaybackPlan) -> Result<()> {
        let hash = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(plan.plan())
                    .map_err(|_| invalid("Plan serialization failed"))?
            )
        );
        if self.source_sha256 != plan.source_sha256() || self.plan_sha256 != hash {
            return Err(invalid("Dry schedule source/plan mismatch"));
        }
        Ok(())
    }
    pub fn notes(&self) -> &[DryNoteReceipt] {
        &self.notes
    }
    /// Applies one owner's dry stereo block, including its released tail. Does
    /// not allocate, mix, truncate, schedule notes, or certify lane ownership.
    /// Both buffers are validated before mutation. Output remains f64 PCM.
    pub fn apply(
        &self,
        note: usize,
        frame: u64,
        left: &mut [f64],
        right: &mut [f64],
    ) -> Result<()> {
        let n = self
            .notes
            .get(note)
            .ok_or_else(|| invalid("Unknown owner"))?;
        let end = frame
            .checked_add(left.len() as u64)
            .ok_or_else(|| invalid("Frame overflow"))?;
        if left.len() != right.len() || frame < n.start_frame || end > 9_007_199_254_740_991 {
            return Err(invalid("Invalid PCM extent"));
        }
        for (i, (l, r)) in left.iter().zip(right.iter()).enumerate() {
            let g = self.gain(n, frame + i as u64);
            if !l.is_finite() || !r.is_finite() || !(l * g).is_finite() || !(r * g).is_finite() {
                return Err(invalid("Nonfinite PCM product"));
            }
        }
        for (i, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
            let g = self.gain(n, frame + i as u64);
            *l *= g;
            *r *= g;
        }
        Ok(())
    }
    fn gain(&self, n: &DryNoteReceipt, frame: u64) -> f64 {
        let offset = if frame >= n.noteoff_frame {
            n.duration_seconds
        } else {
            ((frame - n.start_frame) as f64 / self.sample_rate as f64).min(n.duration_seconds)
        };
        gain_at(&n.original_envelope, offset)
    }
}
fn validate_curve(duration: f64, points: &[EnvelopePoint]) -> Result<()> {
    if !duration.is_finite() || duration <= 0. {
        return Err(invalid("Invalid note duration"));
    }
    let mut previous = 0.;
    for p in points {
        if !p.offset_seconds.is_finite()
            || p.offset_seconds < previous
            || p.offset_seconds > duration
            || !p.gain.is_finite()
            || p.gain < 0.
        {
            return Err(invalid("Invalid envelope"));
        }
        previous = p.offset_seconds;
    }
    Ok(())
}
fn gain_at(points: &[EnvelopePoint], t: f64) -> f64 {
    let (mut at, mut gain) = (0., 1.);
    for p in points {
        if p.offset_seconds > t {
            return gain + (p.gain - gain) * ((t - at) / (p.offset_seconds - at));
        }
        at = p.offset_seconds;
        gain = p.gain;
    }
    gain
}
/// Internal integration seam: native backend supplies *all playing voices*,
/// including released voices, at the exact completed output cursor. Never an
/// externally deserializable or reusable `empty=true` certificate.
pub(crate) trait LaneVoiceState {
    fn completed_frame(&self) -> u64;
    fn playing_voices(&self, lane: usize) -> Result<usize>;
}
#[derive(Debug, Default)]
pub(crate) struct DryLane {
    owner: Option<usize>,
    start: u64,
    noteoff: Option<u64>,
}
impl DryLane {
    pub(crate) fn admit(
        &mut self,
        lane: usize,
        note: usize,
        scheduled: u64,
        voices: &impl LaneVoiceState,
    ) -> Result<()> {
        if voices.completed_frame() != scheduled {
            return Err(invalid("Admission must not move musical onset"));
        }
        if let Some(off) = self.noteoff {
            if off > scheduled {
                return Err(invalid("Previous owner has not reached NoteOff"));
            }
        }
        if self.owner.is_some() && self.noteoff.is_none() {
            return Err(invalid("Previous owner has not received NoteOff"));
        }
        if voices.playing_voices(lane)? != 0 {
            return Err(invalid(
                "Lane occupied, including release tail; more independent capacity required",
            ));
        }
        self.start = scheduled;
        self.owner = Some(note);
        self.noteoff = None;
        Ok(())
    }
    pub(crate) fn noteoff(&mut self, note: usize, frame: u64) -> Result<()> {
        if self.owner != Some(note) || self.noteoff.is_some() || frame < self.start {
            return Err(invalid("NoteOff owner mismatch"));
        }
        self.noteoff = Some(frame);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn point(t: f64, g: f64) -> EnvelopePoint {
        EnvelopePoint {
            offset_seconds: t,
            gain: g,
        }
    }
    #[test]
    fn steps_ramps_tail_and_transactional_pcm() {
        let points = vec![
            point(0., 1.),
            point(0.2, 1.),
            point(0.2, 0.4),
            point(0.4, 0.8),
        ];
        let s = DryEnvelopeSchedule {
            source_sha256: "source".into(),
            plan_sha256: "plan".into(),
            sample_rate: 10,
            tail_policy: DryTailPolicy::HoldLastGainAtNoteOffV1,
            notes: vec![DryNoteReceipt {
                event: "n".into(),
                start_frame: 0,
                noteoff_frame: 5,
                duration_seconds: 0.5,
                original_envelope: points,
            }],
        };
        let mut l = vec![1.; 8];
        let mut r = l.clone();
        s.apply(0, 0, &mut l, &mut r).unwrap();
        assert_eq!(l[1], 1.);
        assert_eq!(l[2], 0.4);
        assert!((l[3] - 0.6).abs() < 1e-15);
        assert_eq!(&l[4..], &[0.8; 4]);
        assert_eq!(l, r);
        let before = l.clone();
        r[7] = f64::NAN;
        assert!(s.apply(0, 0, &mut l, &mut r).is_err());
        assert_eq!(l, before);
        assert!(validate_curve(1., &[point(0.5, 1.), point(0.4, 1.)]).is_err());
    }
    struct Snapshot(u64, usize);
    impl LaneVoiceState for Snapshot {
        fn completed_frame(&self) -> u64 {
            self.0
        }
        fn playing_voices(&self, _: usize) -> Result<usize> {
            Ok(self.1)
        }
    }
    #[test]
    fn released_tail_blocks_reuse_no_timeout_or_delay() {
        let mut lane = DryLane::default();
        lane.admit(0, 1, 0, &Snapshot(0, 0)).unwrap();
        assert!(lane.admit(0, 2, 10, &Snapshot(10, 0)).is_err());
        lane.noteoff(1, 10).unwrap();
        assert!(lane.admit(0, 2, 1000000, &Snapshot(1000000, 1)).is_err());
        assert!(lane.admit(0, 2, 20, &Snapshot(21, 0)).is_err());
        lane.admit(0, 2, 20, &Snapshot(20, 0)).unwrap();
        assert_eq!(lane.owner, Some(2));
        assert!(lane.noteoff(1, 30).is_err());
    }
    #[test]
    #[ignore = "requires explicit local real FluidSynth PCM proof fixture"]
    fn real_two_lane_pcm_reference() {
        let path = std::env::var("SCORESTREAM_DRY_PCM_FIXTURE").unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let pair: Vec<Vec<f64>> = serde_json::from_value(value["pair"].clone()).unwrap();
        let solo: Vec<Vec<f64>> = serde_json::from_value(value["solo"].clone()).unwrap();
        let other: Vec<Vec<f64>> = serde_json::from_value(value["other"].clone()).unwrap();
        let reference: Vec<Vec<f64>> = serde_json::from_value(value["shaped"].clone()).unwrap();
        assert_eq!(&pair[..2], &solo[..2]);
        assert_eq!(&pair[2..], &other[2..]);
        let rate = 48000.;
        let s = DryEnvelopeSchedule {
            source_sha256: "synthetic-native-proof".into(),
            plan_sha256: "not-a-score-plan".into(),
            sample_rate: 48000,
            tail_policy: DryTailPolicy::HoldLastGainAtNoteOffV1,
            notes: vec![DryNoteReceipt {
                event: "lane0".into(),
                start_frame: 0,
                noteoff_frame: 8192,
                duration_seconds: 8192. / rate,
                original_envelope: vec![
                    point(0., 0.9999),
                    point(2001. / rate, 0.9999),
                    point(2001. / rate, 0.4),
                    point(4001. / rate, 0.4),
                    point(6001. / rate, 0.8),
                ],
            }],
        };
        let mut l = pair[0].clone();
        let mut r = pair[1].clone();
        // Deliberately non-quantum block partition: gain must be stream-stable.
        for start in (0..l.len()).step_by(137) {
            let end = (start + 137).min(l.len());
            s.apply(0, start as u64, &mut l[start..end], &mut r[start..end])
                .unwrap();
        }
        let error = l
            .iter()
            .zip(&reference[0])
            .chain(r.iter().zip(&reference[1]))
            .map(|(a, b)| (a - b).abs())
            .fold(0., f64::max);
        assert!(error < 1e-17, "{error}");
        assert_eq!(&pair[2..], &other[2..]);
    }
    #[test]
    fn recipe_binds_exact_source_and_full_plan() {
        use scorestream_app::playback::{PizzicatoSoundPolicy, plan_source_with_pizzicato};
        let source = "!score 1.1 unit=q\n!requires scorestream.words@1\n!requires scorestream.pizzicato@1\n!part p instrument=violin staves=1\n!voice v part=p staff=1\n!voice w part=p staff=1\n@0\n| p m len=4\n+ #m scorestream.words part=p index=0 text=\"pizz.\" pizzicato=\"yes\" pizzicato_at=1\nv C4:2 id=bow\n.\n@1\nw C4:1 id=pluck\n.\n@2\n+ #m scorestream.words part=p index=1 text=\"arco\" pizzicato=\"no\" pizzicato_at=2\nv D4:1 id=arco\n.\n!end at=4\n";
        let compile = |s: &str| {
            plan_source_with_pizzicato(
                s,
                PizzicatoSoundPolicy::GeneralMidiSharedStringsApproximation,
            )
            .unwrap()
        };
        let p = compile(source);
        let recipe =
            DryEnvelopeSchedule::prepare(&p, 48000, DryTailPolicy::HoldLastGainAtNoteOffV1)
                .unwrap();
        recipe.verify_plan(&p).unwrap();
        assert_eq!(recipe.source_sha256(), p.source_sha256());
        let other = compile(&source.replace("D4:1", "E4:1"));
        assert!(recipe.verify_plan(&other).is_err());
        assert_eq!(
            recipe.notes()[0].original_envelope,
            p.plan().notes[0].gain_envelope
        );
    }
}
