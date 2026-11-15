//! The only SFS attribute decoding boundary for shared semantics.
#![allow(clippy::result_large_err)]
use super::*;
use crate::{
    Attributes, Diagnostic, Document, DocumentKind, Rational, Record, RelationPhase, Scalar,
    Severity, SourceRange,
};
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, Diagnostic>;
fn error(message: impl Into<String>, source: SourceRange) -> Diagnostic {
    Diagnostic::error("notation_lowering", message, source)
}
fn text<'a>(a: &'a Attributes, k: &str) -> Option<&'a str> {
    a.get(k).and_then(Scalar::as_str)
}
fn required<'a>(a: &'a Attributes, k: &str, s: SourceRange) -> Result<&'a str> {
    text(a, k).ok_or_else(|| error(format!("Missing typed field {k}"), s))
}
fn number(a: &Attributes, k: &str, default: i64) -> i64 {
    a.get(k).and_then(Scalar::as_i64).unwrap_or(default)
}
fn flag(a: &Attributes, k: &str, default: bool) -> bool {
    a.get(k).and_then(Scalar::as_bool).unwrap_or(default)
}
fn rational(a: &Attributes, k: &str) -> Option<Rational> {
    text(a, k).and_then(|x| x.parse().ok())
}
fn origin(id: impl Into<String>, source: SourceRange) -> Origin {
    Origin {
        object_id: id.into(),
        source,
    }
}
fn dynamic(token: &str, s: SourceRange) -> Result<DynamicSign> {
    use DynamicLevel::*;
    const LEVELS: &[(&str, DynamicLevel)] = &[
        ("pppp", Pppp),
        ("ppp", Ppp),
        ("pp", Pp),
        ("p", P),
        ("mp", Mp),
        ("mf", Mf),
        ("f", F),
        ("ff", Ff),
        ("fff", Fff),
        ("ffff", Ffff),
    ];
    if token == "sfz" {
        return Ok(DynamicSign::Sforzando);
    }
    if token == "fp" {
        return Ok(DynamicSign::FortePiano);
    }
    LEVELS
        .iter()
        .find(|(name, _)| *name == token)
        .map(|(_, level)| DynamicSign::Level(*level))
        .ok_or_else(|| error(format!("Unknown dynamic {token}"), s))
}
fn articulation(token: &str, s: SourceRange) -> Result<Articulation> {
    use Articulation::*;
    [
        ("staccato", Staccato),
        ("staccatissimo", Staccatissimo),
        ("tenuto", Tenuto),
        ("accent", Accent),
        ("strong-accent", StrongAccent),
        ("breath-mark", BreathMark),
    ]
    .into_iter()
    .find(|(name, _)| *name == token)
    .map(|(_, value)| value)
    .ok_or_else(|| error(format!("Unknown articulation {token}"), s))
}
fn beat(token: &str, s: SourceRange) -> Result<Beat> {
    use BeatUnit::*;
    let dotted = token.ends_with('.');
    let unit = [
        ("w", Whole),
        ("h", Half),
        ("q", Quarter),
        ("e", Eighth),
        ("s", Sixteenth),
    ]
    .into_iter()
    .find(|(name, _)| *name == token.trim_end_matches('.'))
    .map(|(_, unit)| unit)
    .ok_or_else(|| error("Invalid beat unit", s))?;
    Ok(Beat { unit, dotted })
}
fn scope(
    target: &str,
    events: &BTreeMap<String, usize>,
    parts: &BTreeMap<String, InstrumentId>,
    voices: &BTreeMap<String, (PartId, u32)>,
    source: SourceRange,
) -> Result<Scope> {
    if target == "score" {
        return Ok(Scope::Score);
    }
    if let Some(id) = target.strip_prefix('#') {
        return events
            .contains_key(id)
            .then(|| Scope::Event(EventId(id.into())))
            .ok_or_else(|| error("Unresolved event scope", source));
    }
    if parts.contains_key(target) {
        return Ok(Scope::Part(PartId(target.into())));
    }
    if voices.contains_key(target) {
        return Ok(Scope::Voice(VoiceId(target.into())));
    }
    if let Some((part, number)) = target.rsplit_once(".s") {
        if parts.contains_key(part) {
            if let Ok(number) = number.parse() {
                return Ok(Scope::Staff(StaffId {
                    part: PartId(part.into()),
                    number,
                }));
            }
        }
    }
    Err(error(format!("Unresolved scope {target}"), source))
}

/// Lower validated SFS wire data without changing its public JSON or text schema.
/// Direct callers also receive semantic validation, so future format adapters do
/// not accidentally bypass cross-reference/range checks by constructing an AST.
pub fn lower_document(doc: &Document) -> std::result::Result<ResolvedScore, Vec<Diagnostic>> {
    let errors: Vec<_> = crate::validate(doc, None)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    lower(doc).map_err(|e| vec![e])
}
fn lower(doc: &Document) -> Result<ResolvedScore> {
    if doc
        .required_extensions
        .iter()
        .any(|e| e == crate::navigation_extension::EXTENSION)
    {
        return Err(Diagnostic::error(
            "navigation.replay_unsupported",
            "Authored repeats require occurrence-based control and relation replay; playing the written sequence once is not allowed",
            doc.source,
        ));
    }
    if doc.document_kind != DocumentKind::Score {
        return Err(error(
            "Resolved playback notation requires a complete score",
            doc.source,
        ));
    }
    let mut parts: BTreeMap<String, InstrumentId> = doc
        .parts
        .iter()
        .map(|p| (p.id.clone(), InstrumentId(p.instrument.clone())))
        .collect();
    let mut voices: BTreeMap<String, (PartId, u32)> = doc
        .voices
        .iter()
        .map(|v| (v.id.clone(), (PartId(v.part.clone()), v.staff)))
        .collect();
    let mut transpositions = BTreeMap::<String, Rational>::new();
    let mut score = ResolvedScore {
        end: doc.end.clone(),
        events: vec![],
        directions: vec![],
        relations: vec![],
        tempos: vec![],
    };
    for frame in &doc.frames {
        for record in &frame.records {
            if let Record::State(state) = record {
                if let Some(value) = text(&state.attributes, "instrument") {
                    parts.insert(state.target.clone(), InstrumentId(value.into()));
                }
                if let (Some(staff), Some((_, current))) = (
                    state.attributes.get("staff").and_then(Scalar::as_i64),
                    voices.get_mut(&state.target),
                ) {
                    *current = staff as u32;
                }
                if let Some(value) = rational(&state.attributes, "transpose_chromatic") {
                    let octaves =
                        Rational::from_integer(number(&state.attributes, "transpose_octaves", 0));
                    transpositions.insert(
                        state.target.clone(),
                        &value + &(&octaves * &Rational::from_integer(12)),
                    );
                }
                if let Some(value) = text(&state.attributes, "tempo") {
                    let (unit, bpm) = value
                        .split_once(':')
                        .ok_or_else(|| error("Invalid tempo", state.source))?;
                    let bpm = bpm
                        .parse()
                        .map_err(|_| error("Invalid tempo BPM", state.source))?;
                    score.tempos.push(TempoChange {
                        at: frame.time.clone(),
                        value: Tempo {
                            beat: beat(unit, state.source)?,
                            bpm,
                        },
                        origin: origin(format!("tempo:{}", frame.time), state.source),
                    });
                }
            }
        }
        for record in &frame.records {
            if let Record::Event(event) = record {
                let id = EventId(
                    event
                        .id()
                        .ok_or_else(|| {
                            error(
                                "Assign a stable event identity before lowering",
                                event.source,
                            )
                        })?
                        .into(),
                );
                let (part, default_staff) = voices
                    .get(&event.voice)
                    .ok_or_else(|| error("Unresolved voice", event.source))?;
                let staff = StaffId {
                    part: part.clone(),
                    number: number(&event.attributes, "staff", i64::from(*default_staff)) as u32,
                };
                let transposition = transpositions
                    .get(&format!("{}.s{}", part.0, staff.number))
                    .or_else(|| transpositions.get(&part.0))
                    .cloned()
                    .unwrap_or_default();
                let sounding_pitch = event.pitch.semitones().map(|p| &p + &transposition);
                let role = match text(&event.attributes, "kind") {
                    Some("cue") => EventRole::Cue,
                    Some("grace") => EventRole::Grace,
                    _ => EventRole::Note,
                };
                let grace = if event.is_grace() {
                    Some(GraceNotation {
                        anchor: EventId(
                            required(&event.attributes, "anchor", event.source)?
                                .trim_start_matches('#')
                                .into(),
                        ),
                        order: number(&event.attributes, "order", 0),
                        slash: flag(&event.attributes, "slash", false),
                    })
                } else {
                    None
                };
                score.events.push(ResolvedEvent {
                    id: id.clone(),
                    voice: VoiceId(event.voice.clone()),
                    part: part.clone(),
                    staff,
                    instrument: parts[&part.0].clone(),
                    start: frame.time.clone(),
                    duration: event.duration.clone(),
                    written_pitch: event.pitch.clone(),
                    sounding_pitch,
                    role,
                    grace,
                    origin: origin(id.0, event.source),
                });
            }
        }
    }
    let events: BTreeMap<String, usize> = score
        .events
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.0.clone(), i))
        .collect();
    let mut relation_groups = BTreeMap::<String, Vec<(Rational, &crate::Relation)>>::new();
    for frame in &doc.frames {
        for record in &frame.records {
            match record {
                Record::Mark(mark) => {
                    let a = &mark.attributes;
                    let s = mark.source;
                    let kind = match mark.kind.as_str() {
                        "dynamic" => DirectionKind::Dynamic {
                            sign: dynamic(required(a, "value", s)?, s)?,
                        },
                        "articulation" => DirectionKind::Articulation {
                            value: articulation(required(a, "value", s)?, s)?,
                        },
                        "words" => DirectionKind::Words {
                            text: required(a, "text", s)?.into(),
                            language: text(a, "language").map(str::to_string),
                        },
                        "metronome" => DirectionKind::Metronome {
                            tempo: Tempo {
                                beat: beat(required(a, "beat", s)?, s)?,
                                bpm: rational(a, "bpm")
                                    .ok_or_else(|| error("Invalid metronome rate", s))?,
                            },
                            text: text(a, "text").map(str::to_string),
                        },
                        "accidental" => DirectionKind::Accidental {
                            value: required(a, "value", s)?.into(),
                            visible: a.get("visible").and_then(Scalar::as_bool),
                            cautionary: flag(a, "cautionary", false),
                            parentheses: flag(a, "parentheses", false),
                            editorial: flag(a, "editorial", false),
                        },
                        "lyric" => DirectionKind::Lyric {
                            verse: number(a, "verse", 1) as u64,
                            text: required(a, "text", s)?.into(),
                            syllabic: text(a, "syllabic").map(str::to_string),
                            language: text(a, "language").map(str::to_string),
                        },
                        "harmony" => DirectionKind::Harmony {
                            root: required(a, "root", s)?.into(),
                            quality: required(a, "kind", s)?.into(),
                            bass: text(a, "bass").map(str::to_string),
                            text: text(a, "text").map(str::to_string),
                        },
                        _ => {
                            return Err(error(
                                format!("No typed lowering for mark {}", mark.kind),
                                s,
                            ));
                        }
                    };
                    let placement = match text(a, "placement") {
                        Some("above") => Some(Placement::Above),
                        Some("below") => Some(Placement::Below),
                        _ => None,
                    };
                    let object_id = text(a, "id").map(str::to_string).unwrap_or_else(|| {
                        format!(
                            "mark:{}:{}:{}:{}",
                            frame.time,
                            mark.target,
                            mark.kind,
                            serde_json::to_string(a).unwrap()
                        )
                    });
                    score.directions.push(Direction {
                        at: frame.time.clone(),
                        end: None,
                        scope: scope(&mark.target, &events, &parts, &voices, s)?,
                        kind,
                        placement,
                        origin: origin(object_id, s),
                    });
                }
                Record::Relation(relation) => {
                    relation_groups
                        .entry(relation.id.clone())
                        .or_default()
                        .push((frame.time.clone(), relation));
                }
                _ => (),
            }
        }
    }
    for (id, mut group) in relation_groups {
        group.sort_by(|a, b| a.0.cmp(&b.0).then(phase(a.1.phase).cmp(&phase(b.1.phase))));
        let first = group[0].1;
        let a = &first.attributes;
        let kind = match first.kind.as_str() {
            "tie" => RelationKind::Tie {
                sound: flag(a, "sound", true),
                print: flag(a, "print", true),
            },
            "slur" => RelationKind::Slur,
            "crescendo" => RelationKind::Hairpin {
                direction: HairpinKind::Crescendo,
            },
            "diminuendo" => RelationKind::Hairpin {
                direction: HairpinKind::Diminuendo,
            },
            "pedal" => RelationKind::Pedal,
            "octave" => RelationKind::Octave {
                octaves: number(a, "octaves", 0),
                sound: flag(a, "sound", false),
                print: flag(a, "print", true),
            },
            "tuplet" => RelationKind::Tuplet {
                actual: number(a, "actual", 0) as u64,
                normal: number(a, "normal", 0) as u64,
            },
            "beam" => RelationKind::Beam {
                level: number(a, "level", 1) as u64,
            },
            "lyric_extend" => RelationKind::LyricExtend {
                verse: number(a, "verse", 1) as u64,
            },
            _ => {
                return Err(error(
                    format!("No typed lowering for relation {}", first.kind),
                    first.source,
                ));
            }
        };
        let anchors = group
            .iter()
            .map(|(at, r)| RelationAnchor {
                event: EventId(r.anchor.trim_start_matches('#').into()),
                at: at.clone(),
                pedal_value: if matches!(kind, RelationKind::Pedal) {
                    Some(rational(&r.attributes, "value").unwrap_or_else(|| {
                        if r.phase == RelationPhase::End {
                            Rational::zero()
                        } else {
                            Rational::one()
                        }
                    }))
                } else {
                    None
                },
                origin: origin(id.clone(), r.source),
            })
            .collect();
        let event = &score.events[*events
            .get(first.anchor.trim_start_matches('#'))
            .ok_or_else(|| error("Unresolved relation anchor", first.source))?];
        let scope = if matches!(kind, RelationKind::Pedal) {
            Scope::Part(event.part.clone())
        } else {
            Scope::Voice(event.voice.clone())
        };
        score.relations.push(ResolvedRelation {
            id: RelationId(id.clone()),
            scope,
            kind,
            anchors,
            origin: origin(id, first.source),
        });
    }
    score
        .events
        .sort_by(|a, b| a.start.cmp(&b.start).then(a.id.cmp(&b.id)));
    score.directions.sort_by(|a, b| {
        a.at.cmp(&b.at)
            .then(a.scope.cmp(&b.scope))
            .then(a.origin.object_id.cmp(&b.origin.object_id))
    });
    Ok(score)
}
fn phase(p: RelationPhase) -> u8 {
    match p {
        RelationPhase::Start => 0,
        RelationPhase::Continue => 1,
        RelationPhase::End => 2,
    }
}
