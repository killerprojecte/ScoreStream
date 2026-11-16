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
    if token == "sf" {
        return Ok(DynamicSign::Sforzato);
    }
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

/// The shared written-state reducer. A performed traversal checkpoints this exact
/// state before entry controls rather than loading a later written snapshot.
#[derive(Clone)]
pub(super) struct PersistentState {
    pub parts: BTreeMap<String, InstrumentId>,
    pub voices: BTreeMap<String, (PartId, u32)>,
    transpositions: BTreeMap<String, Rational>,
}
impl PersistentState {
    pub fn new(doc: &Document) -> Self {
        Self {
            parts: doc
                .parts
                .iter()
                .map(|p| (p.id.clone(), InstrumentId(p.instrument.clone())))
                .collect(),
            voices: doc
                .voices
                .iter()
                .map(|v| (v.id.clone(), (PartId(v.part.clone()), v.staff)))
                .collect(),
            transpositions: BTreeMap::new(),
        }
    }
    pub fn apply(&mut self, state: &crate::State) {
        if let Some(value) = text(&state.attributes, "instrument") {
            self.parts
                .insert(state.target.clone(), InstrumentId(value.into()));
        }
        if let (Some(staff), Some((_, current))) = (
            state.attributes.get("staff").and_then(Scalar::as_i64),
            self.voices.get_mut(&state.target),
        ) {
            *current = staff as u32;
        }
        if let Some(value) = rational(&state.attributes, "transpose_chromatic") {
            let octaves = Rational::from_integer(number(&state.attributes, "transpose_octaves", 0));
            self.transpositions.insert(
                state.target.clone(),
                &value + &(&octaves * &Rational::from_integer(12)),
            );
        }
    }
    pub fn resolve_event(&self, event: &mut ResolvedEvent, explicit_staff: Option<u32>) {
        let (part, default_staff) = &self.voices[&event.voice.0];
        event.part = part.clone();
        event.staff = StaffId {
            part: part.clone(),
            number: explicit_staff.unwrap_or(*default_staff),
        };
        event.instrument = self.parts[&part.0].clone();
        let transpose = self
            .transpositions
            .get(&format!("{}.s{}", part.0, event.staff.number))
            .or_else(|| self.transpositions.get(&part.0))
            .cloned()
            .unwrap_or_default();
        event.sounding_pitch = event.written_pitch.semitones().map(|p| &p + &transpose);
    }
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
    lower(doc, LoweringProfile::Written).map_err(|e| vec![e])
}
/// Private profiles: only the qualified source-to-clock constructor may consume
/// fermata marks, retaining them in its typed sidecar before returning a result.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LoweringProfile {
    Written,
    ExplicitArticulations,
    ExplicitArticulationsGracesAndFermatas,
    Navigation,
    ExplicitFermatas,
    ExplicitGraces,
    ExplicitGracesAndFermatas,
}
/// Refuse authored articulation data even in unchecked or incomplete ASTs.
pub(super) fn reject_authored_articulations(doc: &Document) -> Result<()> {
    if doc.required_extensions.iter().any(|e| e.starts_with("scorestream.articulation@") || e.starts_with("scorestream.notations@"))
        || doc.frames.iter().flat_map(|f| &f.records).any(|r| {
            matches!(r, Record::Mark(mark) if crate::articulation_extension::is_kind(&mark.kind) || crate::notation_extension::is_kind(&mark.kind))
        })
    {
        return Err(Diagnostic::error(
            "articulation.performance_unsupported",
            "Authored articulation fidelity requires a selected explicit performance interpretation; signs, grouping and print attributes must not be silently discarded",
            doc.source,
        ));
    }
    Ok(())
}
pub(super) fn lower(doc: &Document, profile: LoweringProfile) -> Result<ResolvedScore> {
    if !matches!(
        profile,
        LoweringProfile::ExplicitArticulations
            | LoweringProfile::ExplicitArticulationsGracesAndFermatas
    ) {
        reject_authored_articulations(doc)?;
    }
    if doc
        .required_extensions
        .iter()
        .any(|e| e == crate::slur_extension::EXTENSION)
    {
        return Err(Diagnostic::error(
            "slur.performance_unsupported",
            "Authored slur fidelity has no supported performance interpretation; endpoint geometry and phrasing must not be silently discarded",
            doc.source,
        ));
    }
    if crate::instrument_extension::has_midi_intent(doc) {
        return Err(Diagnostic::error(
            "instrument.performance_unsupported",
            "Authored instrument and MIDI intent requires explicit route realization; volume, pan, ports and programs must not be silently ignored",
            doc.source,
        ));
    }
    let graces = matches!(
        profile,
        LoweringProfile::ExplicitGraces
            | LoweringProfile::ExplicitGracesAndFermatas
            | LoweringProfile::ExplicitArticulationsGracesAndFermatas
    );
    if !graces
        && doc
            .required_extensions
            .iter()
            .any(|e| e == crate::grace_extension::EXTENSION)
    {
        return Err(Diagnostic::error(
            "grace.performance_unsupported",
            "Authored grace timing requires an explicit supported realization; grace attacks must not be omitted or played as ordinary notes",
            doc.source,
        ));
    }
    let navigation = profile != LoweringProfile::Written;
    let fermatas = matches!(
        profile,
        LoweringProfile::ExplicitFermatas
            | LoweringProfile::ExplicitGracesAndFermatas
            | LoweringProfile::ExplicitArticulationsGracesAndFermatas
    );
    if !fermatas
        && doc
            .required_extensions
            .iter()
            .any(|e| e == crate::fermata_extension::EXTENSION)
    {
        return Err(Diagnostic::error(
            "fermata.performance_unsupported",
            "Authored fermatas require an explicit hold-duration policy; playing through them without a hold is not allowed",
            doc.source,
        ));
    }
    if !navigation
        && doc
            .required_extensions
            .iter()
            .any(|e| e == crate::navigation_extension::EXTENSION)
    {
        return Err(Diagnostic::error(
            "navigation.replay_unsupported",
            "Authored repeats require perform_document occurrence projection; once-through lower_document is not allowed",
            doc.source,
        ));
    }
    if doc.document_kind != DocumentKind::Score {
        return Err(error(
            "Resolved playback notation requires a complete score",
            doc.source,
        ));
    }
    let mut persistent = PersistentState::new(doc);
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
                persistent.apply(state);
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
                let (part, default_staff) = persistent
                    .voices
                    .get(&event.voice)
                    .ok_or_else(|| error("Unresolved voice", event.source))?;
                let staff = StaffId {
                    part: part.clone(),
                    number: number(&event.attributes, "staff", i64::from(*default_staff)) as u32,
                };
                let transposition = persistent
                    .transpositions
                    .get(&format!("{}.s{}", part.0, staff.number))
                    .or_else(|| persistent.transpositions.get(&part.0))
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
                    instrument: persistent.parts[&part.0].clone(),
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
                Record::State(state) if state.attributes.contains_key("sound_dynamics") => {
                    let percent = match &state.attributes["sound_dynamics"] {
                        Scalar::Integer(n) => Rational::from_integer(*n),
                        Scalar::String(s) => s
                            .parse()
                            .map_err(|_| error("Invalid sound dynamics", state.source))?,
                        _ => return Err(error("Invalid sound dynamics", state.source)),
                    };
                    score.directions.push(Direction {
                        at: frame.time.clone(),
                        end: None,
                        scope: scope(
                            &state.target,
                            &events,
                            &persistent.parts,
                            &persistent.voices,
                            state.source,
                        )?,
                        kind: DirectionKind::SoundDynamics { percent },
                        placement: None,
                        origin: origin(
                            format!("sound_dynamics:{}:{}", frame.time, state.target),
                            state.source,
                        ),
                    });
                }
                Record::Mark(mark)
                    if navigation && mark.kind == crate::navigation_extension::KIND => {}
                // Authored signs are decoded and retained by the qualified
                // constructor, then every visited sign must pass its clock policy.
                Record::Mark(mark) if mark.kind == crate::instrument_extension::KIND => {}
                // Fully validated authored beams are printed notation only. They
                // do not change pitch, measured time, or grace/fermata policy.
                Record::Mark(mark) if mark.kind == crate::beam_extension::KIND => {}
                // Validated authored coordinates are visual metadata, not performance.
                // Validated page credits are authoring-only, not musical directions.
                Record::Mark(mark) if mark.kind == crate::credits::KIND => {}
                Record::Mark(mark) if crate::geometry::is_kind(&mark.kind) => {}
                Record::Mark(mark) if crate::print_layout::is_kind(&mark.kind) => {}
                Record::Mark(mark)
                    if matches!(
                        profile,
                        LoweringProfile::ExplicitArticulations
                            | LoweringProfile::ExplicitArticulationsGracesAndFermatas
                    ) && (crate::articulation_extension::is_kind(&mark.kind)
                        || crate::notation_extension::is_kind(&mark.kind)) => {}
                Record::Mark(mark) if fermatas && mark.kind == crate::fermata_extension::KIND => {}
                Record::Mark(mark) if graces && mark.kind == crate::grace_extension::KIND => {}
                Record::Mark(mark) => {
                    let a = &mark.attributes;
                    let s = mark.source;
                    let kind = match mark.kind.as_str() {
                        "dynamic" => DirectionKind::Dynamic {
                            sign: dynamic(required(a, "value", s)?, s)?,
                            sound: flag(a, "sound", true),
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
                        scope: scope(
                            &mark.target,
                            &events,
                            &persistent.parts,
                            &persistent.voices,
                            s,
                        )?,
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
    if matches!(
        profile,
        LoweringProfile::ExplicitArticulations
            | LoweringProfile::ExplicitArticulationsGracesAndFermatas
    ) {
        super::articulations::append_directions(doc, &mut score)?;
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

#[cfg(test)]
mod authored_slur_guards {
    use super::*;

    #[test]
    fn every_private_profile_rejects_slur_before_other_feature_guards() {
        let mut doc = Document::new(DocumentKind::Score);
        doc.required_extensions = vec![
            crate::slur_extension::EXTENSION.into(),
            crate::grace_extension::EXTENSION.into(),
            crate::fermata_extension::EXTENSION.into(),
            crate::navigation_extension::EXTENSION.into(),
        ];
        for profile in [
            LoweringProfile::Written,
            LoweringProfile::Navigation,
            LoweringProfile::ExplicitFermatas,
            LoweringProfile::ExplicitGraces,
            LoweringProfile::ExplicitGracesAndFermatas,
        ] {
            assert_eq!(
                lower(&doc, profile).unwrap_err().code,
                "slur.performance_unsupported"
            );
        }
    }
}

#[cfg(test)]
mod authored_articulation_guards {
    use super::*;

    #[test]
    fn all_profiles_reject_header_and_each_actual_kind_without_header() {
        let mut documents = Vec::new();
        let mut declared = Document::new(DocumentKind::Score);
        declared
            .required_extensions
            .push(crate::articulation_extension::EXTENSION.into());
        documents.push(declared.clone());
        declared.required_extensions = vec!["scorestream.articulation@2".into()];
        documents.push(declared);
        for kind in ["event", "notations", "group", "item", "future"] {
            let mut doc = Document::new(DocumentKind::Score);
            doc.frames.push(crate::Frame {
                time: Rational::zero(),
                records: vec![Record::Mark(crate::Mark {
                    target: "#a".into(),
                    kind: format!("scorestream.articulation.{kind}"),
                    attributes: Attributes::new(),
                    source: SourceRange::default(),
                })],
                source: SourceRange::default(),
            });
            documents.push(doc);
        }
        for requirement in [
            crate::notation_extension::EXTENSION,
            "scorestream.notations@2",
        ] {
            let mut doc = Document::new(DocumentKind::Score);
            doc.required_extensions.push(requirement.into());
            documents.push(doc);
        }
        for kind in [
            "scorestream.notations",
            crate::notation_extension::EVENT,
            crate::notation_extension::CHILD,
            "scorestream.notations.future",
        ] {
            let mut doc = Document::new(DocumentKind::Score);
            doc.frames.push(crate::Frame {
                time: Rational::zero(),
                records: vec![Record::Mark(crate::Mark {
                    target: "#a".into(),
                    kind: kind.into(),
                    attributes: Attributes::new(),
                    source: SourceRange::default(),
                })],
                source: SourceRange::default(),
            });
            documents.push(doc);
        }
        for doc in documents {
            let before = doc.clone();
            for profile in [
                LoweringProfile::Written,
                LoweringProfile::Navigation,
                LoweringProfile::ExplicitFermatas,
                LoweringProfile::ExplicitGraces,
                LoweringProfile::ExplicitGracesAndFermatas,
            ] {
                assert_eq!(
                    lower(&doc, profile).unwrap_err().code,
                    "articulation.performance_unsupported"
                );
                assert_eq!(
                    super::super::perform::perform(
                        &doc,
                        super::super::PerformanceLimits::default(),
                        profile,
                        None,
                        None,
                    )
                    .unwrap_err()
                    .code,
                    "articulation.performance_unsupported"
                );
            }
            assert_eq!(doc, before);
        }
    }
}
