//! Loss-aware bridge between the SFS 1.1 wire AST and the common score model.
use crate::{Conversion, Format, MappingEffect, MappingReport, ScoreAdapter, validate_score};
use scorestream_core::notation::{EventId, InstrumentId, PartId, RelationId, VoiceId};
use scorestream_core::score::*;
use scorestream_core::{self as core, *};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default)]
pub struct SfsAdapter;
impl ScoreAdapter for SfsAdapter {
    fn read(&self, source: &str) -> Result<Conversion<ScoreDocument>, MappingReport> {
        let doc = scorestream_codec::parse(source).map_err(diagnostics)?;
        from_document(&doc)
    }
    fn write(&self, score: &ScoreDocument) -> Result<Conversion<String>, MappingReport> {
        let result = to_document(score)?;
        Ok(Conversion {
            value: scorestream_codec::canonical(&result.value),
            report: result.report,
        })
    }
}
fn diagnostics(ds: Vec<Diagnostic>) -> MappingReport {
    let mut report = MappingReport::default();
    for diagnostic in ds.into_iter().filter(|d| d.severity == Severity::Error) {
        report.push_source(
            Format::Sfs,
            MappingEffect::Invalid,
            diagnostic.code,
            diagnostic.object_id.unwrap_or_default(),
            diagnostic.message,
            Some(diagnostic.source),
        );
    }
    report
}

fn unsupported(r: &mut MappingReport, feature: &str, path: &str) {
    r.push(
        Format::Sfs,
        MappingEffect::Unsupported,
        feature,
        path,
        "Feature is not represented by the common conversion profile",
    );
}
fn invalid(r: &mut MappingReport, feature: &str, path: &str) {
    r.push(
        Format::Sfs,
        MappingEffect::Invalid,
        feature,
        path,
        "Value cannot be represented without losing information",
    );
}
fn attrs(r: &mut MappingReport, a: &Attributes, allowed: &[&str], path: &str) {
    for key in a.keys() {
        if !allowed.contains(&key.as_str()) {
            unsupported(r, key, path);
        }
    }
}
fn text<'a>(a: &'a Attributes, key: &str) -> Option<&'a str> {
    a.get(key).and_then(Scalar::as_str)
}
fn integer(a: &Attributes, key: &str) -> Option<i64> {
    a.get(key).and_then(Scalar::as_i64)
}
fn unsigned(a: &Attributes, key: &str, r: &mut MappingReport, path: &str) -> u32 {
    match integer(a, key) {
        None => 0,
        Some(n) => match u32::try_from(n) {
            Ok(n) => n,
            Err(_) => {
                invalid(r, key, path);
                0
            }
        },
    }
}
fn note_type(s: &str) -> Option<NoteType> {
    Some(match s {
        "maxima" => NoteType::Maxima,
        "long" => NoteType::Long,
        "breve" => NoteType::Breve,
        "1" => NoteType::Whole,
        "2" => NoteType::Half,
        "4" => NoteType::Quarter,
        "8" => NoteType::Eighth,
        "16" => NoteType::N16,
        "32" => NoteType::N32,
        "64" => NoteType::N64,
        "128" => NoteType::N128,
        "256" => NoteType::N256,
        "512" => NoteType::N512,
        "1024" => NoteType::N1024,
        _ => return None,
    })
}
fn note_text(n: NoteType) -> &'static str {
    match n {
        NoteType::Maxima => "maxima",
        NoteType::Long => "long",
        NoteType::Breve => "breve",
        NoteType::Whole => "1",
        NoteType::Half => "2",
        NoteType::Quarter => "4",
        NoteType::Eighth => "8",
        NoteType::N16 => "16",
        NoteType::N32 => "32",
        NoteType::N64 => "64",
        NoteType::N128 => "128",
        NoteType::N256 => "256",
        NoteType::N512 => "512",
        NoteType::N1024 => "1024",
    }
}
fn written(a: &Attributes, r: &mut MappingReport, path: &str) -> Option<WrittenDuration> {
    let Some(kind) = text(a, "w").and_then(note_type) else {
        if [
            "dots",
            "tm",
            "tm_normal_type",
            "tm_normal_dots",
            "note_size",
        ]
        .iter()
        .any(|k| a.contains_key(*k))
        {
            unsupported(r, "written_without_note_type", path);
        }
        return None;
    };
    let tm = text(a, "tm")
        .and_then(|s| s.split_once(':'))
        .and_then(|(x, y)| match (x.parse::<u32>(), y.parse::<u32>()) {
            (Ok(actual), Ok(normal)) => Some(TimeModification {
                actual,
                normal,
                normal_type: text(a, "tm_normal_type").and_then(note_type),
                normal_dots: unsigned(a, "tm_normal_dots", r, path),
            }),
            _ => {
                invalid(r, "time_modification_range", path);
                None
            }
        });
    Some(WrittenDuration {
        note_type: kind,
        size: text(a, "note_size").and_then(NoteSize::from_name),
        dots: unsigned(a, "dots", r, path),
        time_modification: tm,
    })
}
/// Convert a validated SFS AST. Unsupported content is never silently discarded.
pub fn from_document(doc: &Document) -> Result<Conversion<ScoreDocument>, MappingReport> {
    let mut report = diagnostics(core::validate(doc, None));
    if report.has_errors() {
        return Err(report);
    }
    if doc.document_kind != DocumentKind::Score {
        unsupported(&mut report, "fragment", "document");
    }
    if doc.span.is_some() {
        unsupported(&mut report, "span", "document");
    }
    if doc.required_extensions.iter().any(|e| {
        e != core::navigation_extension::EXTENSION
            && e != core::fermata_extension::EXTENSION
            && e != core::grace_extension::EXTENSION
            && e != core::instrument_extension::EXTENSION
            && e != core::beam_extension::EXTENSION
            && e != core::slur_extension::EXTENSION
            && e != core::articulation_extension::EXTENSION
            && e != core::notation_extension::EXTENSION
            && e != core::geometry::EXTENSION
    }) {
        unsupported(&mut report, "extensions", "document");
    }
    let mut score = ScoreDocument {
        geometry: if doc.required_extensions.iter().any(|e| e == core::geometry::EXTENSION) {
            core::geometry::decode(doc).expect("validated geometry")
        } else {
            Vec::new()
        },
        metadata: doc.metadata.clone(),
        parts: vec![],
        ties: vec![],
        slurs: vec![],
        tempos: vec![],
        metronomes: vec![],
        dynamics: vec![],
        sound_dynamics: vec![],
        fermatas: vec![],
        graces: vec![],
        beams: vec![],
        articulations: vec![],
        authored_articulations: core::articulation_extension::decode(doc)
            .expect("validated authored articulations"),
        notation_order: core::notation_extension::decode(doc).expect("validated notation order"),
        barlines: vec![],
    };
    let accidentals = crate::accidentals::read_marks(doc, &mut report);
    let grace_metadata: BTreeMap<_, _> = doc
        .frames
        .iter()
        .flat_map(|f| &f.records)
        .filter_map(|r| match r {
            Record::Mark(m) if m.kind == core::grace_extension::KIND => Some((
                m.target.trim_start_matches('#'),
                core::grace_extension::decode(m).expect("validated grace timing"),
            )),
            _ => None,
        })
        .collect();
    for p in &doc.parts {
        score.parts.push(ScorePart {
            id: PartId(p.id.clone()),
            name: p.display_name.clone(),
            instrument: Some(InstrumentId(p.instrument.clone())),
            instrument_metadata: doc
                .frames
                .iter()
                .flat_map(|f| &f.records)
                .find_map(|r| match r {
                    Record::Mark(m)
                        if m.kind == core::instrument_extension::KIND && m.target == p.id =>
                    {
                        Some(
                            core::instrument_extension::decode(m)
                                .expect("validated instrument metadata"),
                        )
                    }
                    _ => None,
                }),
            staves: p.staves,
            voices: doc
                .voices
                .iter()
                .filter(|v| v.part == p.id)
                .map(|v| ScoreVoice {
                    id: VoiceId(v.id.clone()),
                    staff: v.staff,
                })
                .collect(),
            measures: vec![],
        });
    }
    // Measure declarations are collected first, independently of record ordering.
    for frame in &doc.frames {
        for record in &frame.records {
            if let Record::Measure(m) = record {
                attrs(&mut report, &m.attributes, &["label", "implicit"], &m.id);
                if let Some(part) = score.parts.iter_mut().find(|p| p.id.0 == m.target) {
                    part.measures.push(ScoreMeasure {
                        id: m.id.clone(),
                        label: text(&m.attributes, "label").unwrap_or(&m.id).into(),
                        start: frame.time.clone(),
                        length: m.length.clone(),
                        implicit: m
                            .attributes
                            .get("implicit")
                            .and_then(Scalar::as_bool)
                            .unwrap_or(false),
                        states: vec![],
                        events: vec![],
                    });
                } else {
                    unsupported(&mut report, "staff_measure", &m.id);
                }
            }
        }
    }
    for p in &mut score.parts {
        p.measures.sort_by(|a, b| a.start.cmp(&b.start));
    }
    let mut tie_groups: BTreeMap<&str, Vec<(&Rational, &Relation)>> = BTreeMap::new();
    let mut slur_groups: BTreeMap<&str, Vec<&Relation>> = BTreeMap::new();
    let mut next_id = 0;
    let mut used: BTreeSet<String> = doc
        .events()
        .filter_map(|(_, e)| e.id().map(str::to_owned))
        .collect();
    // Generated event IDs must not collide with preserved authored identities.
    used.extend(doc.parts.iter().map(|p| p.id.clone()));
    used.extend(doc.voices.iter().map(|v| v.id.clone()));
    for record in doc.frames.iter().flat_map(|f| &f.records) {
        match record {
            Record::Measure(m) => {
                used.insert(m.id.clone());
            }
            Record::Mark(m) => {
                if let Some(id) = m.attributes.get("id").and_then(Scalar::as_str) {
                    used.insert(id.into());
                }
                if let Some(id) = m
                    .attributes
                    .get("source_id")
                    .and_then(Scalar::as_str)
                    .filter(|_| {
                        m.kind == scorestream_core::navigation_extension::KIND
                            || m.kind == core::instrument_extension::KIND
                    })
                {
                    used.insert(id.into());
                }
            }
            Record::Relation(r) => {
                used.insert(r.id.clone());
            }
            _ => {}
        }
    }
    for frame in &doc.frames {
        for record in &frame.records {
            match record {
                Record::Measure(_) => {}
                Record::Mark(m) if m.kind == "accidental" => {}
                Record::Mark(m)
                    if core::articulation_extension::is_kind(&m.kind)
                        || core::notation_extension::is_kind(&m.kind)
                        || core::geometry::is_kind(&m.kind) => {}
                Record::Mark(m) if m.kind == "articulation" => {
                    crate::articulations::read_mark(m, &mut score, &mut report);
                }
                Record::Mark(m) if m.kind == core::beam_extension::KIND => {
                    match core::beam_extension::decode(m) {
                        Ok(beam) => score.beams.push(beam),
                        Err(d) => invalid(&mut report, &d.code, &m.target),
                    }
                }
                Record::Mark(m) if m.kind == core::navigation_extension::KIND => {
                    crate::sfs_navigation::read_mark(m, &mut score, &mut report);
                }
                Record::Mark(m)
                    if m.kind == core::grace_extension::KIND
                        || m.kind == core::instrument_extension::KIND => {}
                Record::Mark(m) if m.kind == core::fermata_extension::KIND => {
                    crate::fermatas::read_mark(m, &mut score, &mut report);
                }
                Record::Mark(m) if m.kind == "dynamic" => {
                    crate::dynamics::read_mark(m, &frame.time, &mut score, &mut report);
                }
                Record::Mark(m) if m.kind == "metronome" => {
                    crate::tempo::read_mark(m, &frame.time, &mut score, &mut report);
                }
                Record::Mark(m) => unsupported(&mut report, &format!("mark:{}", m.kind), &m.target),
                Record::Relation(rel) => {
                    if rel.kind == "slur"
                        && doc
                            .required_extensions
                            .iter()
                            .any(|e| e == core::slur_extension::EXTENSION)
                    {
                        slur_groups.entry(&rel.id).or_default().push(rel);
                        continue;
                    }
                    if rel.kind != "tie" {
                        unsupported(&mut report, &format!("relation:{}", rel.kind), &rel.id);
                        continue;
                    }
                    attrs(&mut report, &rel.attributes, &["sound", "print"], &rel.id);
                    if ["sound", "print"].iter().any(|key| {
                        rel.attributes.get(*key).and_then(Scalar::as_bool) == Some(false)
                    }) {
                        unsupported(&mut report, "tie.sound_notation_mismatch", &rel.id);
                    }
                    tie_groups
                        .entry(&rel.id)
                        .or_default()
                        .push((&frame.time, rel));
                }
                Record::State(s) if s.attributes.contains_key("sound_dynamics") => {
                    attrs(&mut report, &s.attributes, &["sound_dynamics"], &s.target);
                    crate::dynamics::read_state(s, &frame.time, &mut score, &mut report);
                }
                Record::State(s) if s.target == "score" && s.attributes.contains_key("tempo") => {
                    attrs(&mut report, &s.attributes, &["tempo"], &s.target);
                    let (beat, rate) = text(&s.attributes, "tempo")
                        .unwrap()
                        .split_once(':')
                        .unwrap();
                    let value = crate::tempo::parse_beat(beat).unwrap();
                    if beat != "q" {
                        report.push(Format::Sfs, MappingEffect::Normalized, "tempo.unit", frame.time.to_string(), "Playback tempo converted exactly to quarter-note BPM; printed metronome notation is independent");
                    }
                    score.tempos.push(ScoreTempo {
                        at: frame.time.clone(),
                        quarter_bpm: &value.quarters() * &rate.parse::<Rational>().unwrap(),
                    });
                }
                Record::State(s) => {
                    if s.attributes.is_empty() {
                        invalid(&mut report, "empty_state", &s.target);
                    }
                    attrs(
                        &mut report,
                        &s.attributes,
                        &[
                            "key",
                            "meter",
                            "clef",
                            "transpose_chromatic",
                            "transpose_diatonic",
                            "transpose_octaves",
                        ],
                        &s.target,
                    );
                    let Some((part_id, staff)) = s
                        .target
                        .rsplit_once(".s")
                        .and_then(|(p, n)| n.parse::<u32>().ok().map(|n| (p, n)))
                    else {
                        unsupported(&mut report, "state_scope", &s.target);
                        continue;
                    };
                    let measure = score
                        .parts
                        .iter_mut()
                        .find(|p| p.id.0 == part_id)
                        .and_then(|p| p.measures.iter_mut().find(|m| m.start == frame.time));
                    let Some(measure) = measure else {
                        unsupported(&mut report, "state_inside_measure", &s.target);
                        continue;
                    };
                    let mut state = StaffState {
                        staff,
                        clef: None,
                        key: None,
                        meter: None,
                        transpose: None,
                    };
                    if s.attributes.contains_key("key") {
                        match integer(&s.attributes, "key") {
                            Some(fifths) => state.key = Some(KeySignature { fifths }),
                            None => unsupported(&mut report, "key_unset", &s.target),
                        }
                    }
                    if let Some(c) = text(&s.attributes, "clef") {
                        state.clef = match c.as_bytes() {
                            [sign, line] if b"GFC".contains(sign) && line.is_ascii_digit() => {
                                Some(Clef {
                                    sign: match sign {
                                        b'G' => ClefSign::G,
                                        b'F' => ClefSign::F,
                                        _ => ClefSign::C,
                                    },
                                    line: line - b'0',
                                })
                            }
                            _ => {
                                unsupported(&mut report, "clef", &s.target);
                                None
                            }
                        };
                    }
                    if let Some(m) = text(&s.attributes, "meter") {
                        state.meter = m.split_once('/').and_then(|(n, d)| {
                            Some(Meter {
                                beats: n
                                    .split('+')
                                    .map(str::parse)
                                    .collect::<Result<Vec<u32>, _>>()
                                    .ok()?,
                                beat_type: d.parse().ok()?,
                            })
                        });
                        if state.meter.is_none() {
                            unsupported(&mut report, "meter", &s.target);
                        }
                    }
                    if let Some(chromatic) = text(&s.attributes, "transpose_chromatic") {
                        state.transpose = Some(Transposition {
                            chromatic: chromatic.parse().expect("validated rational transposition"),
                            diatonic: integer(&s.attributes, "transpose_diatonic"),
                            octaves: integer(&s.attributes, "transpose_octaves")
                                .expect("validated transposition tuple"),
                        });
                    }
                    if let Some(old) = measure.states.iter_mut().find(|v| v.staff == staff) {
                        if state.clef.is_some() {
                            old.clef = state.clef;
                        }
                        if state.key.is_some() {
                            old.key = state.key;
                        }
                        if state.meter.is_some() {
                            old.meter = state.meter;
                        }
                        if state.transpose.is_some() {
                            old.transpose = state.transpose;
                        }
                    } else {
                        measure.states.push(state);
                    }
                }
                Record::Event(e) => {
                    if e.is_grace() {
                        attrs(
                            &mut report,
                            &e.attributes,
                            &[
                                "id",
                                "staff",
                                "w",
                                "dots",
                                "tm",
                                "tm_normal_type",
                                "tm_normal_dots",
                                "note_size",
                                "kind",
                                "anchor",
                                "order",
                                "slash",
                                "stem",
                            ],
                            e.id().unwrap_or(&e.voice),
                        );
                        if !doc
                            .required_extensions
                            .iter()
                            .any(|x| x == core::grace_extension::EXTENSION)
                        {
                            unsupported(
                                &mut report,
                                "grace.required_fidelity",
                                e.id().unwrap_or(&e.voice),
                            );
                            continue;
                        }
                        let id = e.id().expect("validated grace metadata target");
                        score.graces.push(ScoreGraceNote {
                            id: EventId(id.into()),
                            anchor: EventId(
                                text(&e.attributes, "anchor")
                                    .unwrap()
                                    .trim_start_matches('#')
                                    .into(),
                            ),
                            order: integer(&e.attributes, "order").unwrap(),
                            pitch: e.pitch.clone(),
                            written: written(&e.attributes, &mut report, id),
                            slash: e.attributes.get("slash").and_then(Scalar::as_bool),
                            timing: grace_metadata[id].clone(),
                            stem: text(&e.attributes, "stem").and_then(StemDirection::from_name),
                            accidental: accidentals.get(id).cloned(),
                        });
                        continue;
                    }
                    attrs(
                        &mut report,
                        &e.attributes,
                        &[
                            "id",
                            "chord",
                            "staff",
                            "w",
                            "dots",
                            "tm",
                            "tm_normal_type",
                            "tm_normal_dots",
                            "note_size",
                            "stem",
                            "measure_rest",
                        ],
                        e.id().unwrap_or(&e.voice),
                    );
                    let Some(duration) = e.duration.measured() else {
                        unsupported(&mut report, "grace", &e.voice);
                        continue;
                    };
                    if !matches!(e.pitch, Pitch::Pitched { .. } | Pitch::Rest) {
                        unsupported(&mut report, "pitch_kind", &e.voice);
                    }
                    let Some(voice) = doc.voices.iter().find(|v| v.id == e.voice) else {
                        invalid(&mut report, "voice", &e.voice);
                        continue;
                    };
                    let measure = score
                        .parts
                        .iter_mut()
                        .find(|p| p.id.0 == voice.part)
                        .and_then(|p| {
                            p.measures.iter_mut().find(|m| {
                                m.start <= frame.time && frame.time < &m.start + &m.length
                            })
                        });
                    let Some(measure) = measure else {
                        unsupported(&mut report, "event_without_measure", &e.voice);
                        continue;
                    };
                    let id = if let Some(id) = e.id() {
                        id.to_owned()
                    } else {
                        loop {
                            next_id += 1;
                            let id = format!("event{next_id}");
                            if used.insert(id.clone()) {
                                break id;
                            }
                        }
                    };
                    let staff = if e.attributes.contains_key("staff") {
                        unsigned(&e.attributes, "staff", &mut report, &id)
                    } else {
                        voice.staff
                    };
                    measure.events.push(ScoreEvent {
                        id: EventId(id.clone()),
                        voice: VoiceId(e.voice.clone()),
                        staff,
                        offset: &frame.time - &measure.start,
                        duration: duration.clone(),
                        pitch: e.pitch.clone(),
                        chord: e.chord().map(str::to_owned),
                        written: written(&e.attributes, &mut report, &id),
                        stem: text(&e.attributes, "stem").and_then(StemDirection::from_name),
                        measure_rest: e.attributes.get("measure_rest").and_then(Scalar::as_bool),
                        accidental: accidentals.get(&id).cloned(),
                    });
                }
            }
        }
    }
    for (id, mut group) in tie_groups {
        group.sort_by(|a, b| a.0.cmp(b.0));
        score.ties.push(ScoreTie {
            id: RelationId(id.into()),
            events: group
                .into_iter()
                .map(|(_, rel)| EventId(rel.anchor.trim_start_matches('#').into()))
                .collect(),
        });
    }
    for (id, endpoints) in slur_groups {
        // Native validation has already checked phases, exact onset and ownership.
        // Never merge distinct IDs merely because their endpoints are identical.
        let start = endpoints
            .iter()
            .find(|e| e.phase == RelationPhase::Start)
            .expect("validated slur start");
        let end = endpoints
            .iter()
            .find(|e| e.phase == RelationPhase::End)
            .expect("validated slur end");
        score.slurs.push(ScoreSlur {
            id: RelationId(id.into()),
            start: core::slur_extension::decode_endpoint(start).expect("validated slur endpoint"),
            end: core::slur_extension::decode_endpoint(end).expect("validated slur endpoint"),
        });
    }
    let end = score
        .parts
        .iter()
        .flat_map(|p| &p.measures)
        .map(|m| &m.start + &m.length)
        .max()
        .unwrap_or_else(Rational::zero);
    if end != doc.end {
        unsupported(&mut report, "end_outside_measures", "document");
    }
    crate::tempo::normalize(&mut score.tempos, &mut report, Format::Sfs);
    crate::dynamics::normalize(&mut score, &mut report, Format::Sfs);
    crate::fermatas::sort(&mut score);
    crate::graces::sort(&mut score);
    crate::beams::sort(&mut score);
    crate::slurs::sort(&mut score);
    crate::slurs::authored_only(&score, Format::Sfs, &mut report);
    crate::articulations::sort(&mut score);
    crate::graces::authored_only(&score, Format::Sfs, &mut report);
    crate::fermatas::authored_only(&score, Format::Sfs, &mut report);
    report.append(validate_score(&score));
    crate::sfs_navigation::sort_barlines(&mut score);
    if !report.has_errors() {
        crate::sfs_navigation::check_profile(&score, &mut report);
    }
    report.finish(score)
}

fn put(a: &mut Attributes, key: &str, value: impl Into<String>) {
    a.insert(key.into(), Scalar::String(value.into()));
}
/// Allocate wire-safe IDs without collisions with either original or generated IDs.
struct Ids {
    used: BTreeSet<String>,
    mapping: BTreeMap<String, String>,
    prefix: &'static str,
    next: usize,
}
impl Ids {
    fn new(
        values: impl Iterator<Item = String>,
        prefix: &'static str,
        reserved: &BTreeSet<String>,
    ) -> Self {
        Self {
            used: values
                .chain(reserved.iter().cloned())
                .filter(|v| is_identifier(v))
                .collect(),
            mapping: BTreeMap::new(),
            prefix,
            next: 0,
        }
    }
    fn get(&mut self, id: &str, r: &mut MappingReport) -> String {
        if is_identifier(id) {
            return id.into();
        }
        if let Some(v) = self.mapping.get(id) {
            return v.clone();
        }
        let out = loop {
            self.next += 1;
            let v = format!("{}{}", self.prefix, self.next);
            if self.used.insert(v.clone()) {
                break v;
            }
        };
        r.push(
            Format::Sfs,
            MappingEffect::Normalized,
            "identifier",
            id,
            format!("SFS identifier normalized to {out}"),
        );
        self.mapping.insert(id.into(), out.clone());
        out
    }
}
/// Export through the original codec after common and SFS semantic validation.
pub fn to_document(score: &ScoreDocument) -> Result<Conversion<Document>, MappingReport> {
    let mut report = validate_score(score);
    if !report.has_errors() {
        crate::sfs_navigation::check_profile(score, &mut report);
    }
    if report.has_errors() {
        return Err(report);
    }
    // XML source identities are preserved verbatim. Reserve them before every
    // native namespace allocator can synthesize a normalized identifier.
    let authored_source_ids: BTreeSet<String> = score
        .authored_articulations
        .iter()
        .flat_map(|a| &a.events)
        .flat_map(|e| &e.notations)
        .flat_map(|w| {
            w.source_id
                .iter()
                .chain(w.groups.iter().filter_map(|g| g.source_id.as_ref()))
        })
        .cloned()
        .collect();
    let mut doc = Document::new(DocumentKind::Score);
    if !score.geometry.is_empty() {
        doc.required_extensions
            .push(core::geometry::EXTENSION.into());
    }
    doc.metadata = score.metadata.clone();
    if score.notation_order.is_some() {
        doc.required_extensions
            .push(core::notation_extension::EXTENSION.into());
    }
    if score.authored_articulations.is_some() {
        doc.required_extensions
            .push(core::articulation_extension::EXTENSION.into());
    }
    if score.parts.iter().any(|p| p.instrument_metadata.is_some()) {
        doc.required_extensions
            .push(core::instrument_extension::EXTENSION.into());
    }
    if !score.fermatas.is_empty() {
        doc.required_extensions
            .push(core::fermata_extension::EXTENSION.into());
        crate::fermatas::authored_only(score, Format::Sfs, &mut report);
    }
    if !score.graces.is_empty() {
        doc.required_extensions
            .push(core::grace_extension::EXTENSION.into());
        crate::graces::authored_only(score, Format::Sfs, &mut report);
    }
    if !score.slurs.is_empty() {
        doc.required_extensions
            .push(core::slur_extension::EXTENSION.into());
        crate::slurs::authored_only(score, Format::Sfs, &mut report);
    }
    if !score.beams.is_empty() {
        doc.required_extensions
            .push(core::beam_extension::EXTENSION.into());
    }
    if !score.barlines.is_empty() {
        doc.required_extensions
            .push(core::navigation_extension::EXTENSION.into());
    }
    let mut aliases: Vec<String> = score
        .parts
        .iter()
        .flat_map(|p| {
            std::iter::once(p.id.0.clone()).chain(p.voices.iter().map(|v| v.id.0.clone()))
        })
        .collect();
    aliases.extend(score.barlines.iter().filter_map(|b| b.id.clone()));
    aliases.extend(
        score
            .parts
            .iter()
            .filter_map(|p| p.instrument_metadata.as_ref().map(|m| m.source_id.clone())),
    );
    let mut parts = Ids::new(aliases.iter().cloned(), "part", &authored_source_ids);
    let mut voices = Ids::new(aliases.into_iter(), "voice", &authored_source_ids);
    let mut objects: Vec<String> = score
        .parts
        .iter()
        .flat_map(|p| {
            p.measures.iter().flat_map(|m| {
                std::iter::once(m.id.clone()).chain(m.events.iter().map(|e| e.id.0.clone()))
            })
        })
        .collect();
    objects.extend(score.barlines.iter().filter_map(|b| b.id.clone()));
    objects.extend(score.graces.iter().map(|g| g.id.0.clone()));
    objects.extend(
        score
            .parts
            .iter()
            .filter_map(|p| p.instrument_metadata.as_ref().map(|m| m.source_id.clone())),
    );
    let mut measures = Ids::new(objects.iter().cloned(), "measure", &authored_source_ids);
    let mut events = Ids::new(objects.into_iter(), "event", &authored_source_ids);
    let mut chords = Ids::new(
        score.parts.iter().flat_map(|p| {
            p.measures
                .iter()
                .flat_map(|m| m.events.iter().filter_map(|e| e.chord.clone()))
        }),
        "chord",
        &authored_source_ids,
    );
    let mut frames: BTreeMap<Rational, Vec<Record>> = BTreeMap::new();
    for p in &score.parts {
        let pid = parts.get(&p.id.0, &mut report);
        if let Some(value) = &p.instrument_metadata {
            frames
                .entry(Rational::zero())
                .or_default()
                .push(Record::Mark(core::instrument_extension::encode(
                    pid.clone(),
                    value,
                )));
        }
        let instrument = match &p.instrument {
            Some(i) => i.0.clone(),
            None => {
                report.push(
                    Format::Sfs,
                    MappingEffect::Normalized,
                    "instrument",
                    &p.id.0,
                    "Missing instrument set to explicit unknown",
                );
                "unknown".into()
            }
        };
        doc.parts.push(Part {
            id: pid.clone(),
            instrument,
            staves: p.staves,
            display_name: p.name.clone(),
            source: SourceRange::default(),
        });
        for v in &p.voices {
            doc.voices.push(Voice {
                id: voices.get(&v.id.0, &mut report),
                part: pid.clone(),
                staff: v.staff,
                source: SourceRange::default(),
            });
        }
        for m in &p.measures {
            let mut a = Attributes::new();
            put(&mut a, "label", m.label.clone());
            a.insert("implicit".into(), Scalar::Boolean(m.implicit));
            frames
                .entry(m.start.clone())
                .or_default()
                .push(Record::Measure(Measure {
                    target: pid.clone(),
                    id: measures.get(&m.id, &mut report),
                    length: m.length.clone(),
                    attributes: a,
                    source: SourceRange::default(),
                }));
            for s in &m.states {
                if s.clef.is_none() && s.key.is_none() && s.meter.is_none() && s.transpose.is_none()
                {
                    invalid(&mut report, "empty_state", &m.id);
                    continue;
                }
                let mut a = Attributes::new();
                if let Some(k) = s.key {
                    a.insert("key".into(), Scalar::Integer(k.fifths));
                }
                if let Some(c) = s.clef {
                    put(
                        &mut a,
                        "clef",
                        format!(
                            "{}{}",
                            match c.sign {
                                ClefSign::G => 'G',
                                ClefSign::F => 'F',
                                ClefSign::C => 'C',
                            },
                            c.line
                        ),
                    );
                }
                if let Some(t) = &s.meter {
                    put(
                        &mut a,
                        "meter",
                        format!(
                            "{}/{}",
                            t.beats
                                .iter()
                                .map(u32::to_string)
                                .collect::<Vec<_>>()
                                .join("+"),
                            t.beat_type
                        ),
                    );
                }
                if let Some(t) = &s.transpose {
                    if let Some(diatonic) = t.diatonic {
                        put(&mut a, "transpose_chromatic", t.chromatic.to_string());
                        a.insert("transpose_diatonic".into(), Scalar::Integer(diatonic));
                        a.insert("transpose_octaves".into(), Scalar::Integer(t.octaves));
                    } else {
                        unsupported(&mut report, "transpose.diatonic_unspecified", &m.id);
                    }
                }
                frames
                    .entry(m.start.clone())
                    .or_default()
                    .push(Record::State(State {
                        target: format!("{pid}.s{}", s.staff),
                        attributes: a,
                        source: SourceRange::default(),
                    }));
            }
            for e in &m.events {
                if !matches!(e.pitch, Pitch::Pitched { .. } | Pitch::Rest) {
                    unsupported(&mut report, "pitch_kind", &e.id.0);
                    continue;
                }
                let mut a = Attributes::new();
                let id = events.get(&e.id.0, &mut report);
                crate::accidentals::write_mark(
                    e.accidental.as_ref(),
                    &id,
                    &m.start + &e.offset,
                    &mut frames,
                );
                put(&mut a, "id", id);
                a.insert("staff".into(), Scalar::Integer(i64::from(e.staff)));
                if let Some(c) = &e.chord {
                    put(&mut a, "chord", chords.get(c, &mut report));
                }
                if let Some(stem) = e.stem {
                    put(&mut a, "stem", stem.name());
                }
                if let Some(full) = e.measure_rest {
                    a.insert("measure_rest".into(), Scalar::Boolean(full));
                }
                if let Some(w) = &e.written {
                    put(&mut a, "w", note_text(w.note_type));
                    if let Some(size) = w.size {
                        put(&mut a, "note_size", size.name());
                    }
                    a.insert("dots".into(), Scalar::Integer(i64::from(w.dots)));
                    if let Some(t) = &w.time_modification {
                        put(&mut a, "tm", format!("{}:{}", t.actual, t.normal));
                        if let Some(n) = t.normal_type {
                            put(&mut a, "tm_normal_type", note_text(n));
                        }
                        a.insert(
                            "tm_normal_dots".into(),
                            Scalar::Integer(i64::from(t.normal_dots)),
                        );
                    }
                }
                frames
                    .entry(&m.start + &e.offset)
                    .or_default()
                    .push(Record::Event(Event {
                        voice: voices.get(&e.voice.0, &mut report),
                        pitch: e.pitch.clone(),
                        duration: Duration::Measured(e.duration.clone()),
                        attributes: a,
                        source: SourceRange::default(),
                    }));
            }
            let end = &m.start + &m.length;
            if end > doc.end {
                doc.end = end;
            }
        }
    }
    let grace_anchors = crate::ties::event_index(score);
    for g in &score.graces {
        let anchor = &grace_anchors[&g.anchor];
        let id = events.get(&g.id.0, &mut report);
        crate::accidentals::write_mark(
            g.accidental.as_ref(),
            &id,
            anchor.start.clone(),
            &mut frames,
        );
        let mut a = Attributes::new();
        put(&mut a, "id", id.clone());
        put(&mut a, "kind", "grace");
        put(
            &mut a,
            "anchor",
            format!("#{}", events.get(&g.anchor.0, &mut report)),
        );
        a.insert("order".into(), Scalar::Integer(g.order));
        a.insert(
            "staff".into(),
            Scalar::Integer(i64::from(anchor.event.staff)),
        );
        if let Some(slash) = g.slash {
            a.insert("slash".into(), Scalar::Boolean(slash));
        }
        if let Some(stem) = g.stem {
            put(&mut a, "stem", stem.name());
        }
        if let Some(w) = &g.written {
            put(&mut a, "w", note_text(w.note_type));
            a.insert("dots".into(), Scalar::Integer(i64::from(w.dots)));
            if let Some(size) = w.size {
                put(&mut a, "note_size", size.name());
            }
            if let Some(t) = &w.time_modification {
                put(&mut a, "tm", format!("{}:{}", t.actual, t.normal));
                if let Some(n) = t.normal_type {
                    put(&mut a, "tm_normal_type", note_text(n));
                }
                a.insert(
                    "tm_normal_dots".into(),
                    Scalar::Integer(i64::from(t.normal_dots)),
                );
            }
        }
        frames
            .entry(anchor.start.clone())
            .or_default()
            .push(Record::Event(Event {
                voice: voices.get(&anchor.event.voice.0, &mut report),
                pitch: g.pitch.clone(),
                duration: Duration::Grace,
                attributes: a,
                source: SourceRange::default(),
            }));
        let mut timing = Attributes::new();
        for (key, value) in [
            ("steal_previous", &g.timing.steal_previous),
            ("steal_following", &g.timing.steal_following),
            ("make_time", &g.timing.make_time_quarters),
        ] {
            if let Some(v) = value {
                put(&mut timing, key, v.to_string());
            }
        }
        frames
            .entry(anchor.start.clone())
            .or_default()
            .push(Record::Mark(Mark {
                target: format!("#{id}"),
                kind: core::grace_extension::KIND.into(),
                attributes: timing,
                source: SourceRange::default(),
            }));
    }
    if !score.ties.is_empty() {
        let index = crate::ties::event_index(score);
        let mut relations = Ids::new(
            score
                .ties
                .iter()
                .map(|tie| tie.id.0.clone())
                .chain(score.slurs.iter().map(|slur| slur.id.0.clone())),
            "tie",
            &authored_source_ids,
        );
        for tie in &score.ties {
            let id = relations.get(&tie.id.0, &mut report);
            for (i, event) in tie.events.iter().enumerate() {
                frames
                    .entry(index[event].start.clone())
                    .or_default()
                    .push(Record::Relation(Relation {
                        kind: "tie".into(),
                        id: id.clone(),
                        phase: if i == 0 {
                            RelationPhase::Start
                        } else if i + 1 == tie.events.len() {
                            RelationPhase::End
                        } else {
                            RelationPhase::Continue
                        },
                        anchor: format!("#{}", events.get(&event.0, &mut report)),
                        attributes: Attributes::new(),
                        source: SourceRange::default(),
                    }));
            }
        }
    }
    let mut slur_ids = BTreeMap::new();
    if !score.slurs.is_empty() {
        let positions = crate::beams::positions(score);
        let mut relations = Ids::new(
            score
                .slurs
                .iter()
                .map(|slur| slur.id.0.clone())
                .chain(score.ties.iter().map(|tie| tie.id.0.clone())),
            "slur",
            &authored_source_ids,
        );
        for slur in &score.slurs {
            let id = relations.get(&slur.id.0, &mut report);
            slur_ids.insert(slur.id.0.clone(), id.clone());
            for (phase, endpoint) in [
                (RelationPhase::Start, &slur.start),
                (RelationPhase::End, &slur.end),
            ] {
                frames
                    .entry(positions[&endpoint.event].clone())
                    .or_default()
                    .push(Record::Relation(core::slur_extension::encode_endpoint(
                        id.clone(),
                        events.get(&endpoint.event.0, &mut report),
                        phase,
                        endpoint,
                    )));
            }
        }
    }
    if let Some(authored) = &score.authored_articulations {
        let positions = crate::beams::positions(score);
        for (event_index, event) in authored.events.iter().enumerate() {
            let id = events.get(&event.event.0, &mut report);
            frames
                .entry(positions[&event.event].clone())
                .or_default()
                .extend(
                    core::articulation_extension::encode_event(event, event_index, &id)
                        .into_iter()
                        .map(Record::Mark),
                );
        }
    }
    if let Some(order) = &score.notation_order {
        let positions = crate::beams::positions(score);
        for (index, event) in order.events.iter().enumerate() {
            let mut mapped = event.clone();
            for child in mapped.wrappers.iter_mut().flatten() {
                if let NotationChild::Slur { relation, .. } = child {
                    relation.0 = slur_ids[&relation.0].clone();
                }
            }
            let id = events.get(&event.event.0, &mut report);
            frames
                .entry(positions[&event.event].clone())
                .or_default()
                .extend(
                    core::notation_extension::encode_event(&mapped, index, &id)
                        .into_iter()
                        .map(Record::Mark),
                );
        }
    }
    let geometry_positions = crate::beams::positions(score);
    let geometry_measures: BTreeMap<_, _> = score
        .parts
        .iter()
        .flat_map(|p| {
            p.measures
                .iter()
                .map(move |m| ((p.id.0.as_str(), m.id.as_str()), &m.start))
        })
        .collect();
    for geometry in &score.geometry {
        let (at, host, part) = match geometry {
            core::geometry::Geometry::Event { event, .. } => (
                geometry_positions[&EventId(event.clone())].clone(),
                events.get(event, &mut report),
                None,
            ),
            core::geometry::Geometry::Measure { part, measure, .. } => {
                let at = geometry_measures[&(part.as_str(), measure.as_str())];
                (
                    at.clone(),
                    measures.get(measure, &mut report),
                    Some(parts.get(part, &mut report)),
                )
            }
        };
        frames
            .entry(at)
            .or_default()
            .push(Record::Mark(core::geometry::encode(geometry, host, part)));
    }
    let measure_index: BTreeMap<_, _> = score
        .parts
        .iter()
        .flat_map(|p| p.measures.iter().map(|m| (m.id.as_str(), m)))
        .collect();
    for b in &score.barlines {
        let m = measure_index[b.measure.as_str()];
        let at = if b.location == BarlineLocation::Left {
            m.start.clone()
        } else {
            &m.start + &m.length
        };
        crate::sfs_navigation::write_barline(
            b,
            parts.get(&b.part.0, &mut report),
            measures.get(&b.measure, &mut report),
            at,
            &mut frames,
        );
    }
    for d in &score.dynamics {
        let part = parts.get(&d.scope.part.0, &mut report);
        crate::dynamics::write_mark(d, &part, &mut frames);
    }
    for d in &score.sound_dynamics {
        let part = parts.get(&d.scope.part.0, &mut report);
        crate::dynamics::write_state(d, &part, &mut frames);
    }
    let articulation_positions = crate::fermatas::positions(score);
    for a in &score.articulations {
        let event = events.get(&a.event.0, &mut report);
        crate::articulations::write_mark(
            a,
            event,
            articulation_positions[&a.event].clone(),
            &mut frames,
        );
    }
    let fermata_positions = crate::fermatas::positions(score);
    for f in &score.fermatas {
        let event = events.get(&f.event.0, &mut report);
        crate::fermatas::write_mark(f, event, fermata_positions[&f.event].clone(), &mut frames);
    }
    let beam_positions = crate::beams::positions(score);
    for b in &score.beams {
        let event = events.get(&b.event.0, &mut report);
        frames
            .entry(beam_positions[&b.event].clone())
            .or_default()
            .push(Record::Mark(core::beam_extension::encode(event, b)));
    }
    crate::tempo::write_tempos(score, &mut frames);
    for mark in &score.metronomes {
        let part = parts.get(&mark.part.0, &mut report);
        crate::tempo::write_mark(mark, &part, &mut frames);
    }
    doc.frames = frames
        .into_iter()
        .map(|(time, records)| Frame {
            time,
            records,
            source: SourceRange::default(),
        })
        .collect();
    report.append(diagnostics(core::validate(&doc, None)));
    report.finish(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m len=4 label=\"1\"\n= p.s1 key=0 meter=4/4 clef=G2\nv C4:1 id=e1 chord=c w=4 dots=0\nv E4:2 id=e2 chord=c w=2 dots=0\n.\n@2\nv r:2 id=e3 w=2\n.\n!end at=4\n";
    #[test]
    fn exact_chord_and_states_round_trip() {
        let score = SfsAdapter.read(SOURCE).unwrap().value;
        let m = &score.parts[0].measures[0];
        assert_eq!(m.events[0].duration, Rational::one());
        assert_eq!(m.events[1].duration, Rational::from_integer(2));
        assert_eq!(m.events[2].offset, Rational::from_integer(2));
        let output = SfsAdapter.write(&score).unwrap().value;
        assert_eq!(score, SfsAdapter.read(&output).unwrap().value);
    }
    #[test]
    fn independent_written_tuplet_fields_round_trip() {
        let input = SOURCE.replace(
            "C4:1 id=e1 chord=c w=4 dots=0",
            "C4:1 id=e1 chord=c w=8 dots=1 tm=3:2 tm_normal_type=16 tm_normal_dots=2",
        );
        let score = SfsAdapter.read(&input).unwrap().value;
        let w = score.parts[0].measures[0].events[0]
            .written
            .as_ref()
            .unwrap();
        assert_eq!(w.time_modification.as_ref().unwrap().normal_dots, 2);
        assert_eq!(
            score,
            SfsAdapter
                .read(&SfsAdapter.write(&score).unwrap().value)
                .unwrap()
                .value
        );
    }
    #[test]
    fn reject_unmapped_symbols_instead_of_dropping() {
        for input in [
            SOURCE.replace("id=e3 w=2", "id=e3 w=2 notehead=diamond"),
            SOURCE.replace("@2", "@2\n= p.s1 key=2"),
            SOURCE.replace("id=e3 w=2", "id=e3 w=2\n+ score dynamic value=f"),
        ] {
            assert!(SfsAdapter.read(&input).is_err(), "{input}");
        }
    }
    #[test]
    fn preserve_part_display_name_and_normalize_foreign_ids() {
        let mut score = SfsAdapter.read(SOURCE).unwrap().value;
        score.parts[0].name = Some("A beautiful piano".into());
        let named = SfsAdapter.write(&score).unwrap();
        assert_eq!(
            SfsAdapter.read(&named.value).unwrap().value.parts[0].name,
            score.parts[0].name
        );
        score.parts[0].name = None;
        score.parts[0].measures[0].id = "1".into();
        let result = SfsAdapter.write(&score).unwrap();
        assert!(
            result
                .report
                .diagnostics
                .iter()
                .any(|d| d.effect == MappingEffect::Normalized)
        );
        assert!(SfsAdapter.read(&result.value).is_ok());
    }
    #[test]
    fn unknown_instrument_preserves_independent_display_name() {
        let mut score = SfsAdapter.read(SOURCE).unwrap().value;
        score.parts[0].instrument = None;
        let result = SfsAdapter.write(&score).unwrap();
        assert!(result.value.contains("instrument=unknown"));
        assert!(
            result
                .report
                .diagnostics
                .iter()
                .any(|d| d.feature == "instrument")
        );
        score.parts[0].name = Some("Solo".into());
        let named = SfsAdapter.write(&score).unwrap();
        assert!(named.value.contains("instrument=unknown"));
        assert_eq!(
            SfsAdapter.read(&named.value).unwrap().value.parts[0]
                .name
                .as_deref(),
            Some("Solo")
        );
    }
    #[test]
    fn preserves_measure_boundary_changes_and_additive_meter() {
        let input=SOURCE.replace("!end at=4", "@4\n| p m2 len=3 label=\"2\" implicit=true\n= p.s1 key=-2 meter=2+1/4 clef=F4\nv r:3 id=e4\n.\n!end at=7");
        let score = SfsAdapter.read(&input).unwrap().value;
        assert_eq!(
            score.parts[0].measures[1].states[0]
                .meter
                .as_ref()
                .unwrap()
                .beats,
            vec![2, 1]
        );
        assert_eq!(
            score,
            SfsAdapter
                .read(&SfsAdapter.write(&score).unwrap().value)
                .unwrap()
                .value
        );
    }
    #[test]
    fn rejects_out_of_range_written_fields() {
        let input = SOURCE.replace("id=e3 w=2", "id=e3 w=2 dots=4294967296");
        assert!(
            SfsAdapter
                .read(&input)
                .unwrap_err()
                .diagnostics
                .iter()
                .any(|d| d.effect == MappingEffect::Invalid)
        );
    }
    #[test]
    fn rejects_trailing_duration_and_unmeasured_events() {
        assert!(
            SfsAdapter
                .read(&SOURCE.replace("!end at=4", "!end at=5"))
                .unwrap_err()
                .diagnostics
                .iter()
                .any(|d| d.feature == "end_outside_measures")
        );
        let input = SOURCE
            .replace("| p m len=4 label=\"1\"\n", "")
            .replace("= p.s1 key=0 meter=4/4 clef=G2\n", "");
        assert!(
            SfsAdapter
                .read(&input)
                .unwrap_err()
                .diagnostics
                .iter()
                .any(|d| d.feature == "event_without_measure")
        );
    }
    #[test]
    fn metadata_text_authors_and_ast_provenance_are_preserved() {
        let mut score = SfsAdapter.read(SOURCE).unwrap().value;
        score.metadata = DocumentMetadata {
            title: Some("标题 & <title>\nsecond line".into()),
            description: Some("\u{0001}quoted \"description\"".into()),
            header: Some("页首".into()),
            footer: Some("页尾".into()),
            subtitle: Some("subtitle".into()),
            composer: Some("composer".into()),
            lyricist: Some("lyricist".into()),
            arranger: Some("arranger".into()),
            work_number: Some("Op. 12".into()),
            movement: Some("II".into()),
            movement_number: Some("2a".into()),
            source_description: Some("Édition originale".into()),
            rights: Some("rights".into()),
            language: Some("zh".into()),
            authors: vec!["张三".into(), "A, B".into(), "张三".into()],
            source: BTreeMap::from([("title".into(), SourceRange::line(17, 12))]),
        };
        let doc = to_document(&score).unwrap().value;
        assert_eq!(doc.metadata, score.metadata);
        assert_eq!(from_document(&doc).unwrap().value.metadata, score.metadata);
        let result = SfsAdapter
            .read(&SfsAdapter.write(&score).unwrap().value)
            .unwrap()
            .value;
        // Source ranges describe the newly parsed text, not old source locations.
        assert_eq!(
            serde_json::to_value(result.metadata).unwrap(),
            serde_json::to_value(score.metadata).unwrap()
        );
    }
    #[test]
    fn exported_identifiers_do_not_collide_with_original_other_kinds() {
        let mut score = SfsAdapter.read(SOURCE).unwrap().value;
        let part = &mut score.parts[0];
        part.id = PartId("foreign part".into());
        part.voices[0].id = VoiceId("part1".into());
        for e in &mut part.measures[0].events {
            e.voice = VoiceId("part1".into());
        }
        part.measures[0].id = "foreign measure".into();
        part.measures[0].events[0].id = EventId("measure1".into());
        for e in &mut part.measures[0].events[..2] {
            e.chord = Some("foreign chord".into());
        }
        let result = SfsAdapter.write(&score).unwrap();
        let parsed = SfsAdapter.read(&result.value).unwrap().value;
        assert_eq!(parsed.parts[0].id.0, "part2");
        assert_eq!(parsed.parts[0].measures[0].id, "measure2");
        assert_eq!(parsed.parts[0].measures[0].events[0].id.0, "measure1");
        assert_eq!(
            parsed.parts[0].measures[0].events[0].chord,
            Some("chord1".into())
        );
        assert_eq!(
            parsed.parts[0].measures[0].events[1].chord,
            Some("chord1".into())
        );
        assert_eq!(
            result
                .report
                .diagnostics
                .iter()
                .filter(|d| d.effect == MappingEffect::Normalized)
                .count(),
            3
        );
    }
    #[test]
    fn rejects_empty_states_and_asymmetric_pitch_profiles() {
        let mut score = SfsAdapter.read(SOURCE).unwrap().value;
        score.parts[0].measures[0].states[0] = StaffState {
            staff: 1,
            clef: None,
            key: None,
            meter: None,
            transpose: None,
        };
        assert!(
            SfsAdapter
                .write(&score)
                .unwrap_err()
                .diagnostics
                .iter()
                .any(|d| d.feature == "empty_state" || d.feature == "state.empty")
        );
        score.parts[0].measures[0].states.clear();
        for pitch in [Pitch::Spacer, Pitch::Unpitched("snare".into())] {
            score.parts[0].measures[0].events[2].pitch = pitch;
            assert!(
                SfsAdapter
                    .write(&score)
                    .unwrap_err()
                    .diagnostics
                    .iter()
                    .any(|d| d.feature == "pitch_kind")
            );
        }
        let mut doc = scorestream_codec::parse(SOURCE).unwrap();
        if let Record::State(state) = &mut doc.frames[0].records[1] {
            state.attributes.clear();
        }
        assert!(from_document(&doc).is_err());
    }
}

#[cfg(test)]
mod authored_identity_tests {
    use super::*;
    #[test]
    fn every_native_allocator_reserves_authored_source_ids() {
        let reserved: BTreeSet<String> =
            ["part", "voice", "measure", "event", "chord", "tie", "slur"]
                .into_iter()
                .map(|prefix| format!("{prefix}1"))
                .collect();
        for prefix in ["part", "voice", "measure", "event", "chord", "tie", "slur"] {
            let mut ids = Ids::new(std::iter::once("foreign id".into()), prefix, &reserved);
            let mut report = MappingReport::default();
            assert_eq!(ids.get("foreign id", &mut report), format!("{prefix}2"));
            assert_eq!(ids.get("foreign id", &mut report), format!("{prefix}2"));
        }
    }
}
