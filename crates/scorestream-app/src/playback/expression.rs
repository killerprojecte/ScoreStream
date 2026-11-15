//! Numeric realization of typed expression intervals. No SFS attribute decoding.
use super::{
    EnvelopePoint,
    clock::PlaybackClock,
    diagnostic::{Diagnostics, RealizationEffect},
};
use scorestream_codec::{Rational, notation::*};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn velocity(level: DynamicLevel) -> f64 {
    use DynamicLevel::*;
    match level {
        Pppp => 16.,
        Ppp => 24.,
        Pp => 36.,
        P => 48.,
        Mp => 64.,
        Mf => 80.,
        F => 96.,
        Ff => 112.,
        Fff => 120.,
        Ffff => 127.,
    }
}
struct Curve {
    from: f64,
    to: f64,
}
pub(super) struct Expression {
    pub intervals: ExpressionIntervals,
    curves: BTreeMap<RelationId, Curve>,
    numeric: Option<super::numeric::NumericDynamics>,
    articulations: BTreeMap<EventId, BTreeSet<Articulation>>,
}
impl Expression {
    pub fn new(score: &ResolvedScore, warnings: &mut Diagnostics) -> Result<Self, String> {
        let numeric = super::numeric::NumericDynamics::new(score)?;
        let intervals = resolve_expression(score).map_err(|d| {
            d.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        })?;
        for diagnostic in &intervals.diagnostics {
            let origin = Origin {
                object_id: diagnostic.object_id.clone().unwrap_or_default(),
                source: diagnostic.source,
            };
            warnings.add(
                &diagnostic.code,
                "dynamics.scope",
                RealizationEffect::Omitted,
                &origin,
                &diagnostic.message,
            );
        }
        let events: BTreeMap<_, _> = score.events.iter().map(|e| (&e.id, e)).collect();
        let mut curves = BTreeMap::new();
        for spans in intervals.hairpins.values() {
            for span in spans {
                let start = events[&span.start_event];
                let end = events[&span.end_event];
                let from = intervals
                    .assignment_at(start, &span.start)
                    .map(|d| velocity(d.level))
                    .unwrap_or(80.);
                let endpoint = ExpressionIntervals::scopes(end)
                    .iter()
                    .rev()
                    .find_map(|scope| {
                        intervals
                            .in_scope(scope, &span.notated_end)
                            .filter(|d| d.start == span.notated_end)
                    });
                let to = endpoint.map(|d| velocity(d.level)).unwrap_or_else(|| {
                    warnings.add("hairpin_default_target", "dynamics.hairpin", RealizationEffect::Policy, &span.origin,
                        "No explicit endpoint dynamic; policy uses 24 velocity steps inside the bounded span only.");
                    (from + if span.direction == HairpinKind::Crescendo {24.} else {-24.}).clamp(1.,127.)
                });
                if span.end < span.notated_end {
                    warnings.add("hairpin_terminated", "dynamics.hairpin", RealizationEffect::Policy, &span.origin,
                        "A later same-scope dynamic terminates this curve before its printed endpoint.");
                }
                curves.insert(span.id.clone(), Curve { from, to });
            }
        }
        let mut articulations = BTreeMap::<EventId, BTreeSet<Articulation>>::new();
        for d in &score.directions {
            if matches!(d.kind, DirectionKind::Dynamic { sound: false, .. }) {
                warnings.add(
                    "dynamic_visual_only",
                    "dynamics.symbolic",
                    RealizationEffect::NotationOnly,
                    &d.origin,
                    "Printed dynamic is display-only; no sound is inferred from its sign.",
                );
            }
            if let (Scope::Event(id), DirectionKind::Articulation { value }) = (&d.scope, &d.kind) {
                articulations.entry(id.clone()).or_default().insert(*value);
            }
        }
        Ok(Self {
            intervals,
            curves,
            numeric,
            articulations,
        })
    }
    pub fn value(&self, event: &ResolvedEvent, at: &Rational) -> f64 {
        if let Some(numeric) = &self.numeric {
            return numeric.value(event, at).to_f64();
        }
        let assignment = self.intervals.assignment_at(event, at);
        let base = assignment.map(|d| velocity(d.level)).unwrap_or(80.);
        if let Some(span) = self.intervals.hairpin_at(event, at) {
            if assignment.is_some_and(|d| {
                d.scope.rank() > span.scope.rank() || d.scope == span.scope && d.start > span.start
            }) {
                return base;
            }
            let curve = &self.curves[&span.id];
            let fraction = (&(at - &span.start) / &(&span.notated_end - &span.start))
                .to_f64()
                .clamp(0., 1.);
            return curve.from + (curve.to - curve.from) * fraction;
        }
        base
    }
    pub fn gate(&self, event: &ResolvedEvent) -> Rational {
        let values = self.articulations.get(&event.id);
        let has = |a| values.is_some_and(|v| v.contains(&a));
        let mut gate = if has(Articulation::Tenuto) {
            Rational::one()
        } else {
            "23/25".parse().unwrap()
        };
        for (kind, ratio) in [
            (Articulation::Staccato, "1/2"),
            (Articulation::Staccatissimo, "1/4"),
            (Articulation::BreathMark, "4/5"),
        ] {
            if has(kind) {
                gate = gate.min(ratio.parse().unwrap());
            }
        }
        gate
    }
    pub fn attack(&self, event: &ResolvedEvent) -> f64 {
        self.attack_at(event, &event.start)
    }
    pub fn attack_at(&self, event: &ResolvedEvent, at: &Rational) -> f64 {
        let base = self.value(event, at);
        if self.numeric.is_some() {
            return base;
        }
        let arts = self.articulations.get(&event.id);
        let accent = if arts.is_some_and(|v| v.contains(&Articulation::StrongAccent)) {
            24.
        } else if arts.is_some_and(|v| v.contains(&Articulation::Accent)) {
            16.
        } else {
            0.
        };
        let mut attack = base + accent;
        for scope in ExpressionIntervals::scopes(event).into_iter().rev() {
            if let Some(values) = self.intervals.attacks.get(&scope) {
                let lo = values.partition_point(|v| &v.at < at);
                let hi = values.partition_point(|v| &v.at <= at);
                if lo == hi {
                    continue;
                }
                for transient in &values[lo..hi] {
                    attack = attack.max(match transient.sign {
                        // Legacy performer policy: sf and sfz share attack strength, not spelling.
                        DynamicSign::Sforzando | DynamicSign::Sforzato => 112.,
                        DynamicSign::FortePiano => 96.,
                        DynamicSign::Level(l) => velocity(l),
                    });
                }
                break;
            }
        }
        attack.clamp(1., 127.)
    }
    pub fn envelope(
        &self,
        segments: &[&ResolvedEvent],
        release: &Rational,
        initial: u8,
        tempo: &PlaybackClock<'_>,
    ) -> Result<Vec<EnvelopePoint>, String> {
        self.envelope_from(segments, &segments[0].start, release, initial, tempo)
    }
    pub fn envelope_from(
        &self,
        segments: &[&ResolvedEvent],
        start: &Rational,
        release: &Rational,
        initial: u8,
        tempo: &PlaybackClock<'_>,
    ) -> Result<Vec<EnvelopePoint>, String> {
        if let Some(numeric) = &self.numeric {
            return numeric.envelope(segments, start, release, initial, tempo);
        }
        let event = segments[0];
        let active = |at: &Rational| {
            segments[segments
                .partition_point(|e| e.start <= *at)
                .saturating_sub(1)]
        };
        let start_seconds = tempo.seconds(start)?;
        let duration = tempo.seconds(release)? - start_seconds;
        let mut boundaries = BTreeSet::new();
        let scopes: BTreeSet<_> = segments
            .iter()
            .flat_map(|e| ExpressionIntervals::scopes(e))
            .collect();
        for segment in &segments[1..] {
            boundaries.insert(segment.start.clone());
        }
        for scope in scopes {
            if let Some(values) = self.intervals.assignments.get(&scope) {
                // Include both starts and finite ends so a shadowed ancestor is
                // restored at its CURRENT value, never a cached entry value.
                let lo = values.partition_point(|d| d.end <= *start);
                let hi = values.partition_point(|d| d.start < *release);
                for d in &values[lo..hi] {
                    for at in [&d.start, &d.end] {
                        if at > start && at < release {
                            boundaries.insert(at.clone());
                            let left = at - &"1/1000000000".parse().unwrap();
                            if left > *start {
                                boundaries.insert(left);
                            }
                        }
                    }
                }
            }
            if let Some(values) = self.intervals.hairpins.get(&scope) {
                let lo = values.partition_point(|h| h.end <= *start);
                let hi = values.partition_point(|h| h.start < *release);
                for h in &values[lo..hi] {
                    for at in [&h.start, &h.end] {
                        if at > start && at < release {
                            boundaries.insert(at.clone());
                            let left = at - &"1/1000000000".parse().unwrap();
                            if left > *start {
                                boundaries.insert(left);
                            }
                        }
                    }
                }
            }
        }
        for at in tempo.boundaries(start, release) {
            boundaries.insert(at.clone());
        }
        // Release uses the left-hand musical value, so an excluded terminal
        // anchor cannot change the tail of a note ending at that same instant.
        let left = release - &"1/1000000000".parse().unwrap();
        let left = left.max(start.clone());
        let last_value = self.value(active(&left), &left);
        let mut points = vec![EnvelopePoint {
            offset_seconds: 0.,
            gain: 1.,
        }];
        for at in boundaries {
            points.push(EnvelopePoint {
                offset_seconds: tempo.seconds(&at)? - start_seconds,
                gain: self.value(active(&at), &at) / f64::from(initial),
            });
        }
        points.push(EnvelopePoint {
            offset_seconds: duration,
            gain: last_value / f64::from(initial),
        });
        let base = self.value(event, start);
        if f64::from(initial) > base {
            let first = points
                .iter()
                .skip(1)
                .min_by(|a, b| a.offset_seconds.total_cmp(&b.offset_seconds))
                .unwrap();
            let decay = 0.08_f64.min(duration / 4.).min(first.offset_seconds);
            let mix = if first.offset_seconds > 0. {
                decay / first.offset_seconds
            } else {
                1.
            };
            let gain = base / f64::from(initial) + (first.gain - base / f64::from(initial)) * mix;
            points.push(EnvelopePoint {
                offset_seconds: decay,
                gain,
            });
        }
        points.sort_by(|a, b| a.offset_seconds.total_cmp(&b.offset_seconds));
        Ok(points)
    }
}
