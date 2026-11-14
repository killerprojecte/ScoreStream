//! Deterministic performance interpretation of the validated ScoreStream 1.1 AST.
//! Musical arithmetic stays rational through tempo integration; floating point is
//! used only at the audio boundary. This is a performance policy, not score data.
use scorestream_codec::{
    Attributes, Document, Duration, Rational, Record, Relation, RelationPhase, Scalar,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MAX_NOTES: usize = 100_000;
const MAX_SECONDS: f64 = 21_600.0;
const DEFAULT_VELOCITY: f64 = 80.0;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EnvelopePoint {
    pub offset_seconds: f64,
    /// Multiplier relative to the note's initial MIDI velocity, not absolute gain.
    pub gain: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlaybackNote {
    pub id: String,
    pub part: String,
    pub instrument: String,
    pub voice: String,
    pub start_seconds: f64,
    pub duration_seconds: f64,
    pub midi_note: f64,
    pub frequency_hz: f64,
    pub velocity: u8,
    pub gain_envelope: Vec<EnvelopePoint>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlaybackRoute {
    pub part: String,
    pub instrument: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PlaybackPlan {
    pub duration_seconds: f64,
    pub notes: Vec<PlaybackNote>,
    pub routes: Vec<PlaybackRoute>,
    pub warnings: Vec<String>,
}

/// No workspace mutation or device access. Invalid drafts never play a last-good score.
pub fn plan_source(source: &str) -> Result<PlaybackPlan, String> {
    if source.len() > 4 * 1024 * 1024 {
        return Err("Playback accepts score sources up to 4 MiB".into());
    }
    let doc = scorestream_codec::parse(source).map_err(|diagnostics| {
        diagnostics
            .iter()
            .take(8)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    if doc.document_kind != scorestream_codec::DocumentKind::Score {
        return Err("Playback requires a complete score, not a fragment".into());
    }
    plan(&doc)
}

struct TempoPoint {
    at: Rational,
    seconds: Rational,
    seconds_per_quarter: Rational,
}
struct TempoMap(Vec<TempoPoint>);
impl TempoMap {
    fn new(doc: &Document) -> Result<Self, String> {
        let mut points = vec![TempoPoint {
            at: Rational::zero(),
            seconds: Rational::zero(),
            seconds_per_quarter: "1/2".parse().unwrap(),
        }];
        for frame in &doc.frames {
            for record in &frame.records {
                let Record::State(state) = record else {
                    continue;
                };
                if state.target != "score" {
                    continue;
                }
                let Some(tempo) = string(&state.attributes, "tempo") else {
                    continue;
                };
                let (beat, bpm) = tempo.split_once(':').ok_or("Invalid tempo")?;
                let unit = match beat.trim_end_matches('.') {
                    "w" => "4",
                    "h" => "2",
                    "q" => "1",
                    "e" => "1/2",
                    "s" => "1/4",
                    _ => return Err("Unsupported beat unit".into()),
                }
                .parse::<Rational>()
                .unwrap();
                let beat = if beat.ends_with('.') {
                    &unit * &"3/2".parse().unwrap()
                } else {
                    unit
                };
                let bpm: Rational = bpm.parse().map_err(|_| "Invalid tempo BPM")?;
                let previous = points.last().unwrap();
                let seconds = &previous.seconds
                    + &(&(&frame.time - &previous.at) * &previous.seconds_per_quarter);
                points.push(TempoPoint {
                    at: frame.time.clone(),
                    seconds,
                    seconds_per_quarter: &Rational::from_integer(60) / &(&beat * &bpm),
                });
            }
        }
        Ok(Self(points))
    }
    fn seconds(&self, at: &Rational) -> Result<f64, String> {
        let index = self.0.partition_point(|p| p.at <= *at).saturating_sub(1);
        let point = &self.0[index];
        let exact = &point.seconds + &(&(at - &point.at) * &point.seconds_per_quarter);
        let seconds = exact.to_f64();
        if !seconds.is_finite() || !(0.0..=MAX_SECONDS).contains(&seconds) {
            return Err("Playback timeline must fit in six hours at the specified tempo".into());
        }
        Ok(seconds)
    }
}
fn string<'a>(attrs: &'a Attributes, key: &str) -> Option<&'a str> {
    attrs.get(key).and_then(Scalar::as_str)
}
fn rational(attrs: &Attributes, key: &str) -> Option<Rational> {
    string(attrs, key).and_then(|s| s.parse().ok())
}
fn dynamic(value: &str) -> f64 {
    match value {
        "pppp" => 16.,
        "ppp" => 24.,
        "pp" => 36.,
        "p" => 48.,
        "mp" => 64.,
        "mf" => 80.,
        "f" => 96.,
        "ff" => 112.,
        "fff" => 120.,
        "ffff" => 127.,
        "sfz" => 112.,
        "fp" => 96.,
        _ => DEFAULT_VELOCITY,
    }
}
#[derive(Clone)]
struct RawNote {
    id: String,
    part: String,
    instrument: String,
    voice: String,
    staff: String,
    start: Rational,
    end: Rational,
    pitch: f64,
    gate: Rational,
    accent: f64,
    event_dynamic: Option<String>,
}
type Dynamics = BTreeMap<String, Vec<(Rational, String)>>;
#[derive(Clone)]
struct Hairpin {
    start: Rational,
    end: Rational,
    from: f64,
    to: f64,
}
type Hairpins = BTreeMap<String, Vec<Hairpin>>;
struct Pedal {
    start: Rational,
    end: Rational,
}

fn scoped_dynamic(note: &RawNote, at: &Rational, dynamics: &Dynamics) -> (f64, Rational) {
    let mut result = (DEFAULT_VELOCITY, Rational::zero());
    // More specific scopes win, independently of the order of source records.
    for scope in ["score", &note.part, &note.staff, &note.voice] {
        if let Some(points) = dynamics.get(scope) {
            let index = points.partition_point(|(t, _)| t <= at);
            if index > 0 {
                let (time, value) = &points[index - 1];
                result = (
                    match value.as_str() {
                        "sfz" => DEFAULT_VELOCITY,
                        "fp" => dynamic("p"),
                        value => dynamic(value),
                    },
                    time.clone(),
                );
            }
        }
    }
    result
}
fn expression(note: &RawNote, at: &Rational, dynamics: &Dynamics, hairpins: &Hairpins) -> f64 {
    let (base, changed) = scoped_dynamic(note, at, dynamics);
    if let Some(spans) = hairpins.get(&note.voice) {
        let index = spans.partition_point(|h| h.start <= *at);
        if let Some(span) = index
            .checked_sub(1)
            .map(|i| &spans[i])
            .filter(|h| h.start >= changed)
        {
            if at >= &span.end {
                return span.to;
            }
            let fraction = (&(at - &span.start) / &(&span.end - &span.start))
                .to_f64()
                .clamp(0., 1.);
            return span.from + (span.to - span.from) * fraction;
        }
    }
    base
}
fn phase(phase: RelationPhase) -> u8 {
    match phase {
        RelationPhase::Start => 0,
        RelationPhase::Continue => 1,
        RelationPhase::End => 2,
    }
}

fn plan(doc: &Document) -> Result<PlaybackPlan, String> {
    if doc.event_count() > MAX_NOTES {
        return Err("Playback is limited to 100,000 events per score".into());
    }
    let tempo = TempoMap::new(doc)?;
    let score_end = tempo.seconds(&doc.end)?;
    let mut warnings = BTreeSet::new();
    let mut instruments: BTreeMap<_, _> = doc
        .parts
        .iter()
        .map(|p| (p.id.clone(), p.instrument.clone()))
        .collect();
    let mut voice_staff: BTreeMap<_, _> = doc
        .voices
        .iter()
        .map(|v| (v.id.clone(), (v.part.clone(), v.staff)))
        .collect();
    let mut transpositions = BTreeMap::<String, Rational>::new();
    let mut dynamics = Dynamics::new();
    let mut marks = BTreeMap::<String, Vec<(String, String)>>::new();
    let mut relations = BTreeMap::<String, Vec<(Rational, &Relation)>>::new();
    for frame in &doc.frames {
        for record in &frame.records {
            match record {
                Record::Mark(mark) => {
                    let Some(value) = string(&mark.attributes, "value") else {
                        continue;
                    };
                    if let Some(id) = mark.target.strip_prefix('#') {
                        marks
                            .entry(id.into())
                            .or_default()
                            .push((mark.kind.clone(), value.into()));
                    } else if mark.kind == "dynamic" {
                        dynamics
                            .entry(mark.target.clone())
                            .or_default()
                            .push((frame.time.clone(), value.into()));
                    }
                }
                Record::Relation(relation) => relations
                    .entry(relation.id.clone())
                    .or_default()
                    .push((frame.time.clone(), relation)),
                _ => (),
            }
        }
    }
    // Duplicate same-scope dynamics are ambiguous; never let line order choose sound.
    for values in dynamics.values_mut() {
        values.sort();
        if values
            .windows(2)
            .any(|p| p[0].0 == p[1].0 && p[0].1 != p[1].1)
        {
            return Err("Conflicting dynamics at the same scope and time".into());
        }
        values.dedup();
    }
    let mut notes = Vec::new();
    let mut event_positions = BTreeMap::<String, (Rational, Rational, String, String)>::new();
    for frame in &doc.frames {
        // State is frame-wide, including state records below their events.
        for record in &frame.records {
            let Record::State(state) = record else {
                continue;
            };
            if let Some(instrument) = string(&state.attributes, "instrument") {
                instruments.insert(state.target.clone(), instrument.into());
            }
            if let (Some(staff), Some((_, current))) = (
                state.attributes.get("staff").and_then(Scalar::as_i64),
                voice_staff.get_mut(&state.target),
            ) {
                *current = staff as u32;
            }
            if let Some(chromatic) = rational(&state.attributes, "transpose_chromatic") {
                let octaves = Rational::from_integer(
                    state
                        .attributes
                        .get("transpose_octaves")
                        .and_then(Scalar::as_i64)
                        .unwrap_or(0),
                );
                transpositions.insert(
                    state.target.clone(),
                    &chromatic + &(&octaves * &Rational::from_integer(12)),
                );
            }
        }
        for (ordinal, record) in frame.records.iter().enumerate() {
            let Record::Event(event) = record else {
                continue;
            };
            let (part, staff) = voice_staff
                .get(&event.voice)
                .ok_or("Unknown playback voice")?;
            let id = event
                .id()
                .map(str::to_string)
                .unwrap_or_else(|| format!("{}:{}:{ordinal}", event.voice, frame.time));
            let Duration::Measured(duration) = &event.duration else {
                warnings.insert("Grace notes have no measured duration and are omitted from this playback policy.".into());
                continue;
            };
            let end = &frame.time + duration;
            event_positions.insert(
                id.clone(),
                (
                    frame.time.clone(),
                    end.clone(),
                    event.voice.clone(),
                    part.clone(),
                ),
            );
            if string(&event.attributes, "kind") == Some("cue") {
                warnings.insert("Cue notes are silent in this playback policy.".into());
                continue;
            }
            let Some(pitch) = event.pitch.semitones() else {
                if event.pitch.is_note() {
                    warnings.insert(
                        "Unpitched identities need a percussion map and are currently silent."
                            .into(),
                    );
                }
                continue;
            };
            let staff = event
                .attributes
                .get("staff")
                .and_then(Scalar::as_i64)
                .unwrap_or(i64::from(*staff));
            let staff = format!("{part}.s{staff}");
            let transposition = transpositions
                .get(&staff)
                .or_else(|| transpositions.get(part))
                .cloned()
                .unwrap_or_default();
            // Core semitones are C0 = 0; MIDI is C-1 = 0.
            let pitch = (&(&pitch + &transposition) + &Rational::from_integer(12)).to_f64();
            let mut note = RawNote {
                id,
                part: part.clone(),
                instrument: instruments[part].clone(),
                voice: event.voice.clone(),
                staff,
                start: frame.time.clone(),
                end,
                pitch,
                gate: "23/25".parse().unwrap(),
                accent: 0.,
                event_dynamic: None,
            };
            if let Some(values) = marks.get(&note.id) {
                let mut dynamics_seen = BTreeSet::new();
                for (kind, value) in values {
                    if kind == "dynamic" {
                        dynamics_seen.insert(value.clone());
                        note.event_dynamic = Some(value.clone());
                    }
                    if kind == "articulation" {
                        match value.as_str() {
                            "staccato" => note.gate = note.gate.min("1/2".parse().unwrap()),
                            "staccatissimo" => note.gate = note.gate.min("1/4".parse().unwrap()),
                            "tenuto" if note.gate == "23/25".parse().unwrap() => {
                                note.gate = Rational::one()
                            }
                            "accent" => note.accent = note.accent.max(16.),
                            "strong-accent" => note.accent = note.accent.max(24.),
                            "breath-mark" => note.gate = note.gate.min("4/5".parse().unwrap()),
                            _ => (),
                        }
                    }
                }
                if dynamics_seen.len() > 1 {
                    return Err(format!("Conflicting dynamics on event {}", note.id));
                }
            }
            notes.push(note);
        }
    }
    let note_index: BTreeMap<String, usize> = notes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), i))
        .collect();
    for group in relations.values_mut() {
        group.sort_by(|a, b| a.0.cmp(&b.0).then(phase(a.1.phase).cmp(&phase(b.1.phase))));
    }
    let mut hairpins = Hairpins::new();
    let mut pedal = BTreeMap::<String, Vec<Pedal>>::new();
    let mut tie_edges = BTreeMap::<usize, usize>::new();
    let mut tie_targets = BTreeSet::new();
    let mut groups: Vec<_> = relations.values().collect();
    groups.sort_by(|a, b| a[0].0.cmp(&b[0].0).then(a[0].1.id.cmp(&b[0].1.id)));
    for group in groups {
        let first = group[0].1;
        let last = group.last().unwrap().1;
        let anchor = |r: &Relation| r.anchor.trim_start_matches('#').to_string();
        let Some((start, _, voice, part)) = event_positions.get(&anchor(first)) else {
            continue;
        };
        let Some((end, end_tail, end_voice, end_part)) = event_positions.get(&anchor(last)) else {
            continue;
        };
        match first.kind.as_str() {
            "tie"
                if first
                    .attributes
                    .get("sound")
                    .and_then(Scalar::as_bool)
                    .unwrap_or(true) =>
            {
                for pair in group.windows(2) {
                    let (Some(&a), Some(&b)) = (
                        note_index.get(&anchor(pair[0].1)),
                        note_index.get(&anchor(pair[1].1)),
                    ) else {
                        continue;
                    };
                    if notes[a].part != notes[b].part || notes[a].instrument != notes[b].instrument
                    {
                        return Err("A sounding tie cannot change instrument routes".into());
                    }
                    if tie_edges.insert(a, b).is_some_and(|old| old != b) || !tie_targets.insert(b)
                    {
                        return Err("Ambiguous branching or duplicate sounding ties".into());
                    }
                }
            }
            "crescendo" | "diminuendo" => {
                if voice != end_voice || start >= end {
                    warnings.insert("Hairpins require distinct endpoints in one voice; ambiguous spans are omitted.".into());
                    continue;
                }
                if hairpins
                    .get(voice)
                    .and_then(|h| h.last())
                    .is_some_and(|h| h.end > *start)
                {
                    return Err("Overlapping hairpins in one voice are ambiguous".into());
                }
                let Some(&index) = note_index.get(&anchor(first)) else {
                    continue;
                };
                let note = &notes[index];
                let from = note
                    .event_dynamic
                    .as_deref()
                    .map(dynamic)
                    .unwrap_or_else(|| expression(note, start, &dynamics, &hairpins));
                let (target, changed) = scoped_dynamic(note, end, &dynamics);
                let end_mark = note_index
                    .get(&anchor(last))
                    .and_then(|i| notes[*i].event_dynamic.as_deref());
                let to = if let Some(mark) = end_mark {
                    dynamic(mark)
                } else if changed > *start {
                    target
                } else {
                    warnings.insert("Hairpin without an explicit target dynamic uses a 24-velocity-step change.".into());
                    (from + if first.kind == "crescendo" { 24. } else { -24. }).clamp(1., 127.)
                };
                hairpins.entry(voice.clone()).or_default().push(Hairpin {
                    start: start.clone(),
                    end: end.clone(),
                    from,
                    to,
                });
            }
            "pedal" => {
                if part != end_part {
                    warnings.insert("Cross-part pedal spans are ambiguous and omitted.".into());
                    continue;
                }
                let mut down: Option<Rational> = None;
                for (at, r) in group {
                    let value = rational(&r.attributes, "value").unwrap_or_else(|| {
                        if r.phase == RelationPhase::End {
                            Rational::zero()
                        } else {
                            Rational::one()
                        }
                    });
                    if value != Rational::zero() && value != Rational::one() {
                        warnings.insert("Partial pedal is approximated by a half-depth threshold; half-pedal acoustics are not modeled.".into());
                    }
                    let pressed = r.phase != RelationPhase::End && value >= "1/2".parse().unwrap();
                    match (down.as_ref(), pressed) {
                        (None, true) => down = Some(at.clone()),
                        (Some(s), false) => {
                            pedal.entry(part.clone()).or_default().push(Pedal {
                                start: s.clone(),
                                end: at.clone(),
                            });
                            down = None;
                        }
                        _ => (),
                    }
                }
            }
            "slur" => {
                if voice != end_voice {
                    warnings.insert(
                        "Cross-voice slurs retain notation but do not alter playback gates.".into(),
                    );
                    continue;
                }
                let lo = notes.partition_point(|n| n.start < *start);
                let hi = notes.partition_point(|n| n.start < *end_tail);
                for n in &mut notes[lo..hi] {
                    if n.voice == *voice
                        && n.start >= *start
                        && n.start < *end
                        && n.gate == "23/25".parse().unwrap()
                    {
                        n.gate = Rational::one();
                    }
                }
            }
            "octave" if first.attributes.get("sound").and_then(Scalar::as_bool) == Some(true) => {
                if voice != end_voice {
                    warnings.insert("Cross-voice octave spans are ambiguous and omitted.".into());
                    continue;
                }
                let shift = first
                    .attributes
                    .get("octaves")
                    .and_then(Scalar::as_i64)
                    .unwrap_or(0) as f64
                    * 12.;
                let lo = notes.partition_point(|n| n.start < *start);
                let hi = notes.partition_point(|n| n.start < *end_tail);
                for n in &mut notes[lo..hi] {
                    if n.voice == *voice && n.start >= *start && n.start < *end_tail {
                        n.pitch += shift;
                    }
                }
            }
            _ => (),
        }
    }
    // Merge overlapping pedal intervals once, then locate releases in logarithmic time.
    for spans in pedal.values_mut() {
        spans.sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
        let mut merged = Vec::<Pedal>::new();
        for span in spans.drain(..) {
            if let Some(last) = merged.last_mut().filter(|last| last.end >= span.start) {
                last.end = last.end.clone().max(span.end);
            } else {
                merged.push(span);
            }
        }
        *spans = merged;
    }
    let mut output = Vec::new();
    let mut routes = BTreeSet::new();
    let mut duration_seconds = score_end;
    for (index, note) in notes.iter().enumerate() {
        if tie_targets.contains(&index) {
            continue;
        }
        let mut tail = index;
        let mut visited = BTreeSet::new();
        while let Some(&next) = tie_edges.get(&tail) {
            if !visited.insert(tail) {
                return Err("Cyclic tie graph".into());
            }
            tail = next;
        }
        let last = &notes[tail];
        let mut release = &last.start + &(&(&last.end - &last.start) * &last.gate);
        if let Some(spans) = pedal.get(&note.part) {
            let i = spans.partition_point(|span| span.start <= release);
            if let Some(span) = i
                .checked_sub(1)
                .map(|i| &spans[i])
                .filter(|s| release < s.end)
            {
                release = span.end.clone();
            }
        }
        if !(0.0..=127.0).contains(&note.pitch) || !note.pitch.is_finite() {
            warnings.insert(
                "Pitches outside MIDI 0–127 are omitted from built-in and MIDI playback.".into(),
            );
            continue;
        }
        let start_seconds = tempo.seconds(&note.start)?;
        let end_seconds = tempo.seconds(&release)?;
        let duration = end_seconds - start_seconds;
        if duration < 0.000_001 {
            return Err("A note is shorter than the playback clock can represent".into());
        }
        let base = expression(note, &note.start, &dynamics, &hairpins);
        let mut attack = note.event_dynamic.as_deref().map(dynamic).unwrap_or(base);
        // Scoped sfz/fp accent only the attack exactly at the mark; subsequent notes
        // return to the sustained dynamic specified above.
        for scope in ["score", &note.part, &note.staff, &note.voice] {
            if let Some(values) = dynamics.get(scope) {
                let i = values.partition_point(|(t, _)| *t < note.start);
                if let Some((_, value)) = values
                    .get(i)
                    .filter(|(t, v)| *t == note.start && ["sfz", "fp"].contains(&v.as_str()))
                {
                    attack = dynamic(value);
                }
            }
        }
        let velocity = (attack + note.accent).round().clamp(1., 127.) as u8;
        let mut envelope = vec![EnvelopePoint {
            offset_seconds: 0.,
            gain: 1.,
        }];
        let mut boundaries = BTreeSet::new();
        for scope in ["score", &note.part, &note.staff, &note.voice] {
            if let Some(values) = dynamics.get(scope) {
                let lo = values.partition_point(|(t, _)| *t <= note.start);
                let hi = values.partition_point(|(t, _)| *t < release);
                for (at, _) in &values[lo..hi] {
                    boundaries.insert(at.clone());
                    // Dynamic marks are steps, not invented ramps. A tiny rational
                    // left-limit keeps a step while explicit hairpins remain ramps.
                    let before = at - &"1/1000000000".parse().unwrap();
                    if before > note.start {
                        boundaries.insert(before);
                    }
                }
            }
        }
        if let Some(spans) = hairpins.get(&note.voice) {
            let lo = spans.partition_point(|h| h.end <= note.start);
            let hi = spans.partition_point(|h| h.start < release);
            for h in &spans[lo..hi] {
                for at in [&h.start, &h.end] {
                    if at > &note.start && at < &release {
                        boundaries.insert(at.clone());
                    }
                }
            }
        }
        // Tempo boundaries preserve beat-linear envelopes across tempo changes.
        let lo = tempo.0.partition_point(|p| p.at <= note.start);
        let hi = tempo.0.partition_point(|p| p.at < release);
        for point in &tempo.0[lo..hi] {
            boundaries.insert(point.at.clone());
        }
        boundaries.insert(release.clone());
        let event_level = note
            .event_dynamic
            .as_deref()
            .filter(|v| !["sfz", "fp"].contains(v))
            .map(dynamic);
        for at in boundaries {
            let value = if let Some(level) = event_level {
                level * expression(note, &at, &dynamics, &hairpins) / base.max(1.)
            } else if note.event_dynamic.as_deref() == Some("fp") {
                dynamic("p")
            } else {
                expression(note, &at, &dynamics, &hairpins)
            };
            envelope.push(EnvelopePoint {
                offset_seconds: tempo.seconds(&at)? - start_seconds,
                gain: value / f64::from(velocity),
            });
        }
        if f64::from(velocity) > base || note.event_dynamic.as_deref() == Some("fp") {
            let decay = 0.08_f64.min(duration / 4.);
            let sustain = if note.event_dynamic.as_deref() == Some("fp") {
                dynamic("p")
            } else {
                event_level.unwrap_or(base)
            };
            envelope.push(EnvelopePoint {
                offset_seconds: decay,
                gain: sustain / f64::from(velocity),
            });
            envelope.sort_by(|a, b| a.offset_seconds.total_cmp(&b.offset_seconds));
        }
        duration_seconds = duration_seconds.max(end_seconds);
        routes.insert(PlaybackRoute {
            part: note.part.clone(),
            instrument: note.instrument.clone(),
        });
        output.push(PlaybackNote {
            id: note.id.clone(),
            part: note.part.clone(),
            instrument: note.instrument.clone(),
            voice: note.voice.clone(),
            start_seconds,
            duration_seconds: duration,
            midi_note: note.pitch,
            frequency_hz: 440. * 2_f64.powf((note.pitch - 69.) / 12.),
            velocity,
            gain_envelope: envelope,
        });
    }
    output.sort_by(|a, b| {
        a.start_seconds
            .total_cmp(&b.start_seconds)
            .then(a.part.cmp(&b.part))
            .then(a.voice.cmp(&b.voice))
            .then(a.id.cmp(&b.id))
    });
    Ok(PlaybackPlan {
        duration_seconds,
        notes: output,
        routes: routes.into_iter().collect(),
        warnings: warnings.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests;
