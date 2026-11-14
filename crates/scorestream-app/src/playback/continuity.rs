//! Typed continuity, pedal and pitch-span realization, independent of expression.
use super::{
    diagnostic::{Diagnostics, RealizationEffect},
    expression::Expression,
};
use scorestream_codec::{Rational, notation::*};
use std::collections::{BTreeMap, BTreeSet};
struct Pedal {
    start: Rational,
    end: Rational,
}
pub(super) struct Continuity {
    pub ties: BTreeMap<usize, usize>,
    pub targets: BTreeSet<usize>,
    pub gates: Vec<Rational>,
    pub pitch: Vec<Option<Rational>>,
    pedals: BTreeMap<PartId, Vec<Pedal>>,
}
impl Continuity {
    pub fn new(
        score: &ResolvedScore,
        expression: &Expression,
        warnings: &mut Diagnostics,
    ) -> Result<Self, String> {
        let indices: BTreeMap<_, _> = score
            .events
            .iter()
            .enumerate()
            .map(|(i, e)| (e.id.clone(), i))
            .collect();
        let mut out = Self {
            ties: BTreeMap::new(),
            targets: BTreeSet::new(),
            gates: score.events.iter().map(|e| expression.gate(e)).collect(),
            pitch: score
                .events
                .iter()
                .map(|e| e.sounding_pitch.clone())
                .collect(),
            pedals: BTreeMap::new(),
        };
        for relation in &score.relations {
            let Some(first) = relation.anchors.first() else {
                continue;
            };
            let Some(last) = relation.anchors.last() else {
                continue;
            };
            let a = indices[&first.event];
            let b = indices[&last.event];
            let start = &score.events[a];
            let end = &score.events[b];
            match relation.kind {
                RelationKind::Tie { sound: true, .. } => {
                    for pair in relation.anchors.windows(2) {
                        let a = indices[&pair[0].event];
                        let b = indices[&pair[1].event];
                        if score.events[a].part != score.events[b].part
                            || score.events[a].instrument != score.events[b].instrument
                        {
                            return Err("A sounding tie cannot change instrument routes".into());
                        }
                        if out.ties.insert(a, b).is_some_and(|old| old != b)
                            || !out.targets.insert(b)
                        {
                            return Err("Ambiguous branching or duplicate sounding ties".into());
                        }
                    }
                }
                RelationKind::Slur => {
                    if start.voice != end.voice {
                        warnings.add(
                            "slur_cross_voice",
                            "phrasing.slur",
                            RealizationEffect::Omitted,
                            &relation.origin,
                            "Cross-voice phrasing is retained but the playback gate is unchanged.",
                        );
                        continue;
                    }
                    let lo = score.events.partition_point(|e| e.start < start.start);
                    let hi = score.events.partition_point(|e| e.start < end.start);
                    for (i, event) in score.events.iter().enumerate().take(hi).skip(lo) {
                        if event.voice == start.voice && out.gates[i] == "23/25".parse().unwrap() {
                            out.gates[i] = Rational::one();
                        }
                    }
                }
                RelationKind::Pedal => {
                    if start.part != end.part {
                        warnings.add(
                            "pedal_cross_part",
                            "pedal.sustain",
                            RealizationEffect::Omitted,
                            &relation.origin,
                            "Ambiguous cross-part pedal span omitted.",
                        );
                        continue;
                    }
                    let mut down: Option<Rational> = None;
                    for (i, anchor) in relation.anchors.iter().enumerate() {
                        let value = anchor.pedal_value.as_ref().cloned().unwrap_or_default();
                        if value != Rational::zero() && value != Rational::one() {
                            warnings.add("pedal_partial", "pedal.sustain", RealizationEffect::Policy, &relation.origin, "Partial pedal uses a half-depth threshold; acoustic half-pedaling is not modeled.");
                        }
                        let pressed =
                            i + 1 < relation.anchors.len() && value >= "1/2".parse().unwrap();
                        match (down.as_ref(), pressed) {
                            (None, true) => down = Some(anchor.at.clone()),
                            (Some(at), false) => {
                                out.pedals
                                    .entry(start.part.clone())
                                    .or_default()
                                    .push(Pedal {
                                        start: at.clone(),
                                        end: anchor.at.clone(),
                                    });
                                down = None;
                            }
                            _ => (),
                        }
                    }
                }
                RelationKind::Octave {
                    octaves,
                    sound: true,
                    ..
                } => {
                    if start.voice != end.voice {
                        warnings.add(
                            "octave_cross_voice",
                            "pitch.octave",
                            RealizationEffect::Omitted,
                            &relation.origin,
                            "Ambiguous cross-voice sounding octave span omitted.",
                        );
                        continue;
                    }
                    let shift = &Rational::from_integer(octaves) * &Rational::from_integer(12);
                    let lo = score.events.partition_point(|e| e.start < start.start);
                    let hi = score.events.partition_point(|e| e.start < end.end());
                    for (i, event) in score.events.iter().enumerate().take(hi).skip(lo) {
                        if event.voice == start.voice {
                            out.pitch[i] = out.pitch[i].as_ref().map(|p| p + &shift);
                        }
                    }
                }
                _ => (),
            }
        }
        for spans in out.pedals.values_mut() {
            spans.sort_by(|a, b| a.start.cmp(&b.start));
            let mut merged = Vec::<Pedal>::new();
            for span in spans.drain(..) {
                if let Some(last) = merged.last_mut().filter(|s| s.end >= span.start) {
                    last.end = last.end.clone().max(span.end);
                } else {
                    merged.push(span);
                }
            }
            *spans = merged;
        }
        Ok(out)
    }
    pub fn chain(&self, root: usize) -> Result<Vec<usize>, String> {
        let mut chain = vec![root];
        let mut visited = BTreeSet::new();
        let mut current = root;
        while let Some(&next) = self.ties.get(&current) {
            if !visited.insert(current) {
                return Err("Cyclic sounding tie graph".into());
            }
            chain.push(next);
            current = next;
        }
        Ok(chain)
    }
    pub fn release(&self, event: &ResolvedEvent, index: usize) -> Rational {
        let mut release = &event.start
            + &(&event.duration.measured().cloned().unwrap_or_default() * &self.gates[index]);
        if let Some(spans) = self.pedals.get(&event.part) {
            let i = spans.partition_point(|s| s.start <= release);
            if let Some(span) = i
                .checked_sub(1)
                .map(|i| &spans[i])
                .filter(|s| release < s.end)
            {
                release = span.end.clone();
            }
        }
        release
    }
}
