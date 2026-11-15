//! Exact explicit sound controls. Symbolic expression policy cannot override them.
use super::{EnvelopePoint, clock::PlaybackClock};
use scorestream_codec::{Rational, notation::*};
use std::collections::{BTreeMap, BTreeSet};

struct Assignment {
    at: Rational,
    end: Option<Rational>,
    velocity: Rational,
}
pub(super) struct NumericDynamics {
    assignments: BTreeMap<Scope, Vec<Assignment>>,
}
impl NumericDynamics {
    pub fn new(score: &ResolvedScore) -> Result<Option<Self>, String> {
        if !score
            .directions
            .iter()
            .any(|d| matches!(d.kind, DirectionKind::SoundDynamics { .. }))
        {
            return Ok(None);
        }
        if score.directions.iter().any(|d| {
            matches!(
                d.kind,
                DirectionKind::Dynamic { sound: true, .. }
                    | DirectionKind::Articulation {
                        value: Articulation::Accent | Articulation::StrongAccent
                    }
            )
        }) || score
            .relations
            .iter()
            .any(|r| matches!(r.kind, RelationKind::Hairpin { .. }))
        {
            return Err("Explicit sound dynamics cannot yet be combined with sounding symbolic dynamics, hairpins, or attack-changing accents".into());
        }
        let mut assignments = BTreeMap::<Scope, Vec<Assignment>>::new();
        for d in &score.directions {
            let DirectionKind::SoundDynamics { percent } = &d.kind else {
                continue;
            };
            if !matches!(d.scope, Scope::Part(_) | Scope::Staff(_))
                || d.at.is_negative()
                || d.at > score.end
                || d.end
                    .as_ref()
                    .is_some_and(|end| end <= &d.at || end > &score.end)
            {
                return Err("Invalid sound dynamics scope or interval".into());
            }
            // MusicXML's default forte is 90, so percent * 90 / 100 is exact.
            let velocity = percent * &"9/10".parse().unwrap();
            let float = velocity.to_f64();
            if velocity.is_negative()
                || velocity > Rational::from_integer(127)
                || !float.is_finite()
                || (velocity.is_positive() && float == 0.)
            {
                return Err(
                    "Sound dynamics outside representable MIDI-relative range 0–127".into(),
                );
            }
            assignments
                .entry(d.scope.clone())
                .or_default()
                .push(Assignment {
                    at: d.at.clone(),
                    end: d.end.clone(),
                    velocity,
                });
        }
        for values in assignments.values_mut() {
            values.sort_by(|a, b| a.at.cmp(&b.at));
            for pair in values.windows(2) {
                if pair[0].at == pair[1].at
                    && (pair[0].velocity != pair[1].velocity || pair[0].end != pair[1].end)
                {
                    return Err("Conflicting simultaneous sound dynamics in the same scope".into());
                }
            }
            values.dedup_by(|a, b| a.at == b.at);
        }
        Ok(Some(Self { assignments }))
    }
    pub fn value(&self, event: &ResolvedEvent, at: &Rational) -> Rational {
        for scope in [
            Scope::Staff(event.staff.clone()),
            Scope::Part(event.part.clone()),
        ] {
            let Some(values) = self.assignments.get(&scope) else {
                continue;
            };
            let Some(index) = values.partition_point(|v| &v.at <= at).checked_sub(1) else {
                continue;
            };
            let v = &values[index];
            if v.end.as_ref().is_none_or(|end| at < end) {
                return v.velocity.clone();
            }
        }
        Rational::from_integer(80)
    }
    pub fn envelope(
        &self,
        segments: &[&ResolvedEvent],
        release: &Rational,
        initial: u8,
        tempo: &PlaybackClock<'_>,
    ) -> Result<Vec<EnvelopePoint>, String> {
        let start = &segments[0].start;
        let start_seconds = tempo.seconds(start)?;
        let duration = tempo.seconds(release)? - start_seconds;
        let active = |at: &Rational| {
            segments[segments
                .partition_point(|e| &e.start <= at)
                .saturating_sub(1)]
        };
        let mut boundaries = BTreeSet::new();
        let mut scopes = BTreeSet::new();
        for segment in segments {
            boundaries.insert(segment.start.clone());
            scopes.insert(Scope::Staff(segment.staff.clone()));
            scopes.insert(Scope::Part(segment.part.clone()));
        }
        for scope in scopes {
            if let Some(values) = self.assignments.get(&scope) {
                let lo = values.partition_point(|v| &v.at <= start).saturating_sub(1);
                let hi = values.partition_point(|v| &v.at < release);
                for value in &values[lo..hi] {
                    boundaries.insert(value.at.clone());
                    if let Some(end) = &value.end {
                        boundaries.insert(end.clone());
                    }
                }
            }
        }
        let divisor = Rational::from_integer(i64::from(initial));
        let gain = |at: &Rational| (&self.value(active(at), at) / &divisor).to_f64();
        let mut current = gain(start);
        let mut points = vec![EnvelopePoint {
            offset_seconds: 0.,
            gain: current,
        }];
        // Two ordered points at a boundary form a step, never an interpolated
        // crescendo or an epsilon ramp. Time conversion happens only here.
        for at in boundaries
            .into_iter()
            .filter(|at| at > start && at < release)
        {
            let next = gain(&at);
            if next != current {
                let offset = tempo.seconds(&at)? - start_seconds;
                if offset <= points.last().unwrap().offset_seconds || offset >= duration {
                    return Err(
                        "Sound dynamics boundary cannot be represented by the playback clock"
                            .into(),
                    );
                }
                points.push(EnvelopePoint {
                    offset_seconds: offset,
                    gain: current,
                });
                points.push(EnvelopePoint {
                    offset_seconds: offset,
                    gain: next,
                });
                current = next;
            }
        }
        points.push(EnvelopePoint {
            offset_seconds: duration,
            gain: current,
        });
        Ok(points)
    }
}

#[cfg(test)]
mod tests;
