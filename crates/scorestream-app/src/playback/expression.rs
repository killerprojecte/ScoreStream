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
        Self::new_selected(score, warnings, false)
    }
    pub(super) fn new_selected(
        score: &ResolvedScore,
        warnings: &mut Diagnostics,
        simple_accent: bool,
    ) -> Result<Self, String> {
        let numeric = if simple_accent {
            super::numeric::NumericDynamics::new_selected(score, true)?
        } else {
            super::numeric::NumericDynamics::new(score)?
        };
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
            if matches!(
                &d.kind,
                DirectionKind::Dynamic {
                    sound: false,
                    qualifier: Some(_),
                    ..
                }
            ) {
                warnings.add("dynamic_qualifier_not_realized", "dynamics.qualifier", RealizationEffect::NotationOnly, &d.origin,
                    "Authored qualifier text and order are retained; sound=false leaves numeric playback unchanged, without interpreting qualifier expression.");
            }
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
        if simple_accent && numeric.is_some() {
            for event in &score.events {
                if articulations
                    .get(&event.id)
                    .is_some_and(|a| a.contains(&Articulation::Accent))
                {
                    if event.attack_dynamics.is_some() {
                        return Err(format!(
                            "practice.numeric_accent_note_on_conflict: {} has both authored NoteOn dynamics and accent",
                            event.id
                        ));
                    }
                    warnings.add("practice.numeric_simple_accent", "dynamics.articulation", RealizationEffect::Policy, &event.origin,
                        "Explicit editorial SimpleStaccatoAccentV1: a fresh non-muted numeric attack receives +16 velocity steps (clamped), then returns to its unchanged authored numeric baseline within min(0.08 seconds, duration/4, next numeric step). Tie continuations create no attack.");
                }
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
            return if base > 0.
                && self
                    .articulations
                    .get(&event.id)
                    .is_some_and(|a| a.contains(&Articulation::Accent))
            {
                (base + 16.).min(127.)
            } else {
                base
            };
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
            let mut points = numeric.envelope(segments, start, release, initial, tempo)?;
            let base = numeric.value(segments[0], start).to_f64();
            if base > 0.
                && self
                    .articulations
                    .get(&segments[0].id)
                    .is_some_and(|a| a.contains(&Articulation::Accent))
                && f64::from(initial) > base
            {
                let duration = tempo.seconds(release)? - tempo.seconds(start)?;
                let first = points
                    .get(1)
                    .ok_or("Missing numeric envelope endpoint")?
                    .offset_seconds;
                let decay = 0.08_f64.min(duration / 4.).min(first);
                if decay <= 0. {
                    return Err("Numeric accent decay is not representable".into());
                }
                // Retain every authored paired step, including one coincident with decay.
                let baseline = points[0].gain;
                points[0].gain = 1.;
                if decay < first {
                    points.insert(
                        1,
                        EnvelopePoint {
                            offset_seconds: decay,
                            gain: baseline,
                        },
                    );
                }
            }
            return Ok(points);
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

#[cfg(test)]
mod numeric_accent_tests {
    use super::*;
    fn score(percent: &str) -> ResolvedScore {
        let doc=scorestream_codec::parse("!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m len=4\nv C4:4 id=a\n+ #a articulation value=accent\n.\n!end at=4\n").unwrap();
        let mut score = lower_document(&doc).unwrap();
        score.directions.push(Direction {
            at: Rational::zero(),
            end: None,
            scope: Scope::Part(PartId("p".into())),
            kind: DirectionKind::SoundDynamics {
                percent: percent.parse().unwrap(),
            },
            placement: None,
            origin: score.events[0].origin.clone(),
        });
        score
    }
    #[test]
    fn explicit_numeric_accent_returns_to_exact_baseline_and_retains_steps() {
        let mut score = score("80");
        let mut step = score.directions.last().unwrap().clone();
        step.at = "1/100".parse().unwrap();
        step.kind = DirectionKind::SoundDynamics {
            percent: "40".parse().unwrap(),
        };
        score.directions.push(step);
        assert!(Expression::new(&score, &mut Diagnostics::default()).is_err());
        let mut diagnostics = Diagnostics::default();
        let e = Expression::new_selected(&score, &mut diagnostics, true).unwrap();
        let note = &score.events[0];
        assert_eq!(e.attack(note), 88.);
        assert_eq!(e.value(note, &Rational::zero()), 72.);
        let clock = PlaybackClock::new(&score, None).unwrap();
        let points = e.envelope(&[note], &note.end(), 88, &clock).unwrap();
        let original = e
            .numeric
            .as_ref()
            .unwrap()
            .envelope(&[note], &note.start, &note.end(), 88, &clock)
            .unwrap();
        assert_eq!(points[0].gain, 1.);
        assert_eq!(points[1].gain, 72. / 88.);
        assert_eq!(points[1].offset_seconds, original[1].offset_seconds);
        assert_eq!(&points[1..], &original[1..]);
    }
    #[test]
    fn numeric_accent_short_decay_and_saturation_leave_baseline_unchanged() {
        for (percent, velocity) in [("80", 88.), ("140", 127.)] {
            let s = score(percent);
            let e = Expression::new_selected(&s, &mut Diagnostics::default(), true).unwrap();
            let n = &s.events[0];
            let clock = PlaybackClock::new(&s, None).unwrap();
            assert_eq!(e.attack(n), velocity);
            for end in ["4", "1/100"] {
                let end: Rational = end.parse().unwrap();
                let p = e.envelope(&[n], &end, velocity as u8, &clock).unwrap();
                assert_eq!(p[0].gain, 1.);
                let duration = clock.seconds(&end).unwrap();
                assert_eq!(p[1].offset_seconds, 0.08_f64.min(duration / 4.));
                assert_eq!(p[1].gain, e.value(n, &n.start) / velocity);
                assert_eq!(p.last().unwrap().gain, p[1].gain);
            }
        }
    }
    #[test]
    fn numeric_accent_mute_conflict_and_other_expression_stay_closed() {
        let mut s = score("0");
        let e = Expression::new_selected(&s, &mut Diagnostics::default(), true).unwrap();
        assert_eq!(e.attack(&s.events[0]), 0.);
        s.events[0].attack_dynamics = Some(Rational::from_integer(100));
        assert!(
            Expression::new_selected(&s, &mut Diagnostics::default(), true)
                .err()
                .unwrap()
                .contains("note_on_conflict")
        );
        s.events[0].attack_dynamics = None;
        for d in &mut s.directions {
            if matches!(d.kind, DirectionKind::Articulation { .. }) {
                d.kind = DirectionKind::Articulation {
                    value: Articulation::StrongAccent,
                };
            }
        }
        assert!(Expression::new_selected(&s, &mut Diagnostics::default(), true).is_err());
    }
}
