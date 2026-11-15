//! Authored printed beams, validated identically for common scores and native SFS.
//! This capability adds no sounding durations, inferred groups, or playback policy.
#![allow(clippy::result_large_err)]
use crate::{
    Attributes, Diagnostic, Document, DocumentKind, Mark, Pitch, Rational, Record, Scalar,
    SourceRange,
    notation::EventId,
    score::{
        BeamValue, NoteSize, NoteType, ScoreBeam, ScoreDocument, StemDirection, WrittenDuration,
    },
};
use std::collections::BTreeMap;

pub const EXTENSION: &str = "scorestream.beam@1";
pub const KIND: &str = "scorestream.beam";

fn error(mark: &Mark, code: &str, message: &str) -> Diagnostic {
    Diagnostic::error(code, message, mark.source)
}

/// Decode authored fields only. Full validation also checks the capability,
/// target ownership, onset, written flags, and all intervening events.
pub fn decode(mark: &Mark) -> Result<ScoreBeam, Diagnostic> {
    if mark.kind != KIND {
        return Err(error(
            mark,
            "beam.kind",
            "Expected the registered beam kind",
        ));
    }
    if mark
        .attributes
        .keys()
        .any(|k| !["number", "value"].contains(&k.as_str()))
    {
        return Err(error(mark, "beam.field", "Unknown authored-beam field"));
    }
    let event = mark
        .target
        .strip_prefix('#')
        .filter(|id| crate::is_identifier(id))
        .ok_or_else(|| error(mark, "beam.event", "Beam target must be an event reference"))?;
    let number = match mark.attributes.get("number") {
        None => None,
        Some(Scalar::Integer(n)) if (1..=8).contains(n) => Some(*n as u8),
        _ => {
            return Err(error(
                mark,
                "beam.number",
                "Beam number must be an integer from 1 through 8",
            ));
        }
    };
    let value = mark
        .attributes
        .get("value")
        .and_then(Scalar::as_str)
        .and_then(|value| match value {
            "begin" => Some(BeamValue::Begin),
            "continue" => Some(BeamValue::Continue),
            "end" => Some(BeamValue::End),
            "forward-hook" => Some(BeamValue::ForwardHook),
            "backward-hook" => Some(BeamValue::BackwardHook),
            _ => None,
        })
        .ok_or_else(|| {
            error(
                mark,
                "beam.value",
                "Beam value must be begin, continue, end, forward-hook, or backward-hook",
            )
        })?;
    Ok(ScoreBeam {
        event: EventId(event.into()),
        number,
        value,
    })
}

/// Encode using the caller's mapped native event ID (without a # prefix).
/// An absent number remains absent, rather than becoming an explicit level 1.
/// Native hook tokens are atoms; MusicXML and the common model keep their spaced
/// lexical values. This mapping never changes generic or legacy SFS quoting.
pub fn encode(event: String, beam: &ScoreBeam) -> Mark {
    let value = match beam.value {
        BeamValue::ForwardHook => "forward-hook",
        BeamValue::BackwardHook => "backward-hook",
        other => other.name(),
    };
    let mut attributes = BTreeMap::from([("value".into(), Scalar::String(value.into()))]);
    if let Some(number) = beam.number {
        attributes.insert("number".into(), Scalar::Integer(i64::from(number)));
    }
    Mark {
        target: format!("#{event}"),
        kind: KIND.into(),
        attributes,
        source: SourceRange::default(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Lane {
    // Actual measure identity, not its printed label or nominal meter.
    measure: usize,
    voice: String,
    staff: i64,
    grace_anchor: Option<String>,
}
#[derive(Clone)]
struct Context {
    id: Option<String>,
    lane: Option<Lane>,
    at: Rational,
    duration: Option<Rational>,
    within_measure: bool,
    // None is measured; Some(None) is an invalid, missing grace order.
    order: Option<Option<i64>>,
    flags: u8,
    pitched: bool,
    eligible: bool,
    chord: Option<String>,
    written: Option<WrittenRhythm>,
    // The outer None is malformed native spelling; Some(None) is authored absence.
    stem: Option<Option<StemDirection>>,
    source: SourceRange,
}

/// Compare semantic written fields, not serialized spelling or a reduced ratio.
/// Separate arbitrary-precision integers keep 6:4 distinct from 3:2 while
/// normalizing lexical variants of either integer. No fields are inferred.
#[derive(Clone, PartialEq, Eq)]
struct WrittenRhythm {
    note_type: NoteType,
    size: Option<NoteSize>,
    dots: i64,
    time_modification: Option<(Rational, Rational, Option<NoteType>, i64)>,
}
impl WrittenRhythm {
    fn common(written: &WrittenDuration) -> Option<Self> {
        let time_modification = match &written.time_modification {
            Some(tm) if tm.actual > 0 && tm.normal > 0 => Some((
                Rational::from_integer(i64::from(tm.actual)),
                Rational::from_integer(i64::from(tm.normal)),
                tm.normal_type,
                i64::from(tm.normal_dots),
            )),
            Some(_) => return None,
            None => None,
        };
        Some(Self {
            note_type: written.note_type,
            size: written.size,
            dots: i64::from(written.dots),
            time_modification,
        })
    }

    fn native(attributes: &Attributes) -> Option<Self> {
        fn dots(attributes: &Attributes, key: &str) -> Option<i64> {
            match attributes.get(key) {
                None => Some(0),
                Some(value) => value.as_i64().filter(|n| *n >= 0),
            }
        }
        fn integer(value: &str) -> Option<Rational> {
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            value
                .trim_start_matches('0')
                .parse::<Rational>()
                .ok()
                .filter(Rational::is_positive)
        }
        let time_modification = match attributes.get("tm") {
            Some(value) => {
                let (actual, normal) = value.as_str()?.split_once(':')?;
                let normal_type = match attributes.get("tm_normal_type") {
                    Some(value) => Some(native_note_type(value.as_str()?)?),
                    None => None,
                };
                Some((
                    integer(actual)?,
                    integer(normal)?,
                    normal_type,
                    dots(attributes, "tm_normal_dots")?,
                ))
            }
            None => {
                if attributes.contains_key("tm_normal_type")
                    || attributes.contains_key("tm_normal_dots")
                {
                    return None;
                }
                None
            }
        };
        Some(Self {
            note_type: native_note_type(attributes.get("w")?.as_str()?)?,
            size: match attributes.get("note_size") {
                Some(value) => Some(NoteSize::from_name(value.as_str()?)?),
                None => None,
            },
            dots: dots(attributes, "dots")?,
            time_modification,
        })
    }
}

fn native_note_type(value: &str) -> Option<NoteType> {
    Some(match value {
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
impl Context {
    fn error(&self, code: &str, message: &str) -> Diagnostic {
        let mut diagnostic = Diagnostic::error(code, message, self.source);
        diagnostic.object_id = self.id.clone();
        diagnostic
    }
}
fn flags(value: NoteType) -> u8 {
    match value {
        NoteType::Eighth => 1,
        NoteType::N16 => 2,
        NoteType::N32 => 3,
        NoteType::N64 => 4,
        NoteType::N128 => 5,
        NoteType::N256 => 6,
        NoteType::N512 => 7,
        NoteType::N1024 => 8,
        _ => 0,
    }
}
fn native_flags(value: Option<&str>) -> u8 {
    match value {
        Some("8") => 1,
        Some("16") => 2,
        Some("32") => 3,
        Some("64") => 4,
        Some("128") => 5,
        Some("256") => 6,
        Some("512") => 7,
        Some("1024") => 8,
        _ => 0,
    }
}

/// Validate only the authored-beam profile, not the remainder of a common score.
/// All measured and grace events participate, including unmarked events inside
/// a run. Callers must also apply their normal whole-score validation.
pub fn validate_score(score: &ScoreDocument) -> Vec<Diagnostic> {
    if score.beams.is_empty() {
        return vec![];
    }
    let mut contexts = Vec::new();
    let mut measure_index = 0;
    for part in &score.parts {
        for measure in &part.measures {
            for event in &measure.events {
                let owned = measure.length.is_positive()
                    && !event.offset.is_negative()
                    && event.offset < measure.length
                    && event.staff > 0
                    && event.staff <= part.staves
                    && part.voices.iter().any(|v| v.id == event.voice);
                contexts.push(Context {
                    id: Some(event.id.0.clone()),
                    lane: owned.then(|| Lane {
                        measure: measure_index,
                        voice: event.voice.0.clone(),
                        staff: i64::from(event.staff),
                        grace_anchor: None,
                    }),
                    at: &measure.start + &event.offset,
                    duration: Some(event.duration.clone()),
                    within_measure: event.duration.is_positive()
                        && &event.offset + &event.duration <= measure.length,
                    order: None,
                    flags: event.written.as_ref().map_or(0, |w| flags(w.note_type)),
                    pitched: matches!(event.pitch, Pitch::Pitched { .. }),
                    eligible: matches!(event.pitch, Pitch::Pitched { .. } | Pitch::Rest)
                        && event.measure_rest != Some(true),
                    chord: event.chord.clone(),
                    written: event.written.as_ref().and_then(WrittenRhythm::common),
                    stem: Some(event.stem),
                    source: SourceRange::default(),
                });
            }
            measure_index += 1;
        }
    }
    let principals: BTreeMap<_, _> = contexts
        .iter()
        .enumerate()
        .filter_map(|(i, c)| c.id.as_ref().map(|id| (id.clone(), i)))
        .collect();
    for grace in &score.graces {
        let principal = principals.get(&grace.anchor.0).map(|i| &contexts[*i]);
        let mut lane = principal.and_then(|p| p.lane.clone());
        if let Some(lane) = &mut lane {
            lane.grace_anchor = Some(grace.anchor.0.clone());
        }
        contexts.push(Context {
            id: Some(grace.id.0.clone()),
            lane,
            at: principal.map_or_else(Rational::zero, |p| p.at.clone()),
            duration: None,
            within_measure: true,
            order: Some(Some(grace.order)),
            flags: grace.written.as_ref().map_or(0, |w| flags(w.note_type)),
            pitched: matches!(grace.pitch, Pitch::Pitched { .. }),
            eligible: matches!(grace.pitch, Pitch::Pitched { .. }) && grace.timing.valid(),
            chord: None,
            written: grace.written.as_ref().and_then(WrittenRhythm::common),
            stem: Some(grace.stem),
            source: SourceRange::default(),
        });
    }
    validate_contexts(
        &contexts,
        score.beams.iter().map(|b| (b, SourceRange::default())),
    )
}

/// Native normalization deliberately includes effective voice staff state and
/// applies all state records at an onset before reading any event in that frame.
fn native_contexts(doc: &Document) -> Vec<Context> {
    let voices: BTreeMap<_, _> = doc.voices.iter().map(|v| (v.id.as_str(), v)).collect();
    let parts: BTreeMap<_, _> = doc
        .parts
        .iter()
        .map(|p| (p.id.as_str(), p.staves))
        .collect();
    let mut staff: BTreeMap<_, _> = doc
        .voices
        .iter()
        .map(|v| (v.id.as_str(), i64::from(v.staff)))
        .collect();
    let mut measures: BTreeMap<&str, BTreeMap<&Rational, (usize, Rational)>> = BTreeMap::new();
    let mut measure_index = 0;
    for frame in &doc.frames {
        for record in &frame.records {
            if let Record::Measure(m) = record {
                measures
                    .entry(&m.target)
                    .or_default()
                    .insert(&frame.time, (measure_index, &frame.time + &m.length));
                measure_index += 1;
            }
        }
    }
    let has_grace_capability = doc
        .required_extensions
        .iter()
        .any(|e| e == crate::grace_extension::EXTENSION);
    let mut contexts = Vec::new();
    for frame in &doc.frames {
        for record in &frame.records {
            if let Record::State(s) = record {
                if let Some(value) = s.attributes.get("staff").and_then(Scalar::as_i64) {
                    if let Some(current) = staff.get_mut(s.target.as_str()) {
                        *current = value;
                    }
                }
            }
        }
        for record in &frame.records {
            let Record::Event(event) = record else {
                continue;
            };
            let current_staff = event
                .attributes
                .get("staff")
                .and_then(Scalar::as_i64)
                .or_else(|| staff.get(event.voice.as_str()).copied());
            let ownership = voices.get(event.voice.as_str()).and_then(|voice| {
                let current_staff = current_staff?;
                if current_staff <= 0
                    || parts
                        .get(voice.part.as_str())
                        .is_none_or(|n| current_staff > i64::from(*n))
                {
                    return None;
                }
                let staff_target = format!("{}.s{}", voice.part, current_staff);
                let mut owners = [voice.part.as_str(), staff_target.as_str()]
                    .into_iter()
                    .filter_map(|target| {
                        let (_, (index, end)) = measures
                            .get(target)?
                            .range::<Rational, _>(..=&frame.time)
                            .next_back()?;
                        if frame.time >= *end {
                            return None;
                        }
                        Some((*index, end))
                    });
                let owner = owners.next()?;
                if owners.next().is_some() {
                    return None;
                }
                Some((
                    Lane {
                        measure: owner.0,
                        voice: event.voice.clone(),
                        staff: current_staff,
                        grace_anchor: event.is_grace().then(|| {
                            event
                                .attributes
                                .get("anchor")
                                .and_then(Scalar::as_str)
                                .and_then(|s| s.strip_prefix('#'))
                                .unwrap_or("")
                                .into()
                        }),
                    },
                    event
                        .duration
                        .measured()
                        .is_none_or(|d| d.is_positive() && &frame.time + d <= *owner.1),
                ))
            });
            let kind = event.attributes.get("kind").and_then(Scalar::as_str);
            let eligible = if event.is_grace() {
                has_grace_capability
                    && kind == Some("grace")
                    && matches!(event.pitch, Pitch::Pitched { .. })
                    && event.chord().is_none()
            } else {
                matches!(event.pitch, Pitch::Pitched { .. } | Pitch::Rest)
                    && kind.is_none_or(|k| k == "note")
            } && event
                .attributes
                .get("measure_rest")
                .and_then(Scalar::as_bool)
                != Some(true);
            contexts.push(Context {
                id: event.id().map(str::to_owned),
                lane: ownership.as_ref().map(|(lane, _)| lane.clone()),
                at: frame.time.clone(),
                within_measure: ownership.is_some_and(|(_, within)| within),
                duration: event.duration.measured().cloned(),
                order: event
                    .is_grace()
                    .then(|| event.attributes.get("order").and_then(Scalar::as_i64)),
                flags: native_flags(event.attributes.get("w").and_then(Scalar::as_str)),
                pitched: matches!(event.pitch, Pitch::Pitched { .. }),
                eligible,
                chord: event.chord().map(str::to_owned),
                written: WrittenRhythm::native(&event.attributes),
                stem: match event.attributes.get("stem") {
                    Some(value) => value.as_str().and_then(StemDirection::from_name).map(Some),
                    None => Some(None),
                },
                source: event.source,
            });
        }
    }
    contexts
}

pub(crate) fn validate(doc: &Document, out: &mut Vec<Diagnostic>) {
    let enabled = doc.required_extensions.iter().any(|s| s == EXTENSION);
    let marks: Vec<_> = doc
        .frames
        .iter()
        .flat_map(|f| {
            f.records.iter().filter_map(move |r| match r {
                Record::Mark(m) if m.kind == KIND => Some((&f.time, m)),
                _ => None,
            })
        })
        .collect();
    if !enabled && marks.is_empty() {
        return;
    }
    if enabled && marks.is_empty() {
        out.push(Diagnostic::error(
            "beam.empty_requirement",
            "Beam capability requires at least one authored beam",
            doc.source,
        ));
    }
    if !enabled {
        out.push(Diagnostic::error(
            "beam.requirement",
            "Authored beams require !requires scorestream.beam@1",
            doc.source,
        ));
    }
    if doc.document_kind != DocumentKind::Score {
        out.push(Diagnostic::error(
            "beam.document",
            "Authored beams require a complete score",
            doc.source,
        ));
    }
    if doc
        .frames
        .iter()
        .flat_map(|f| &f.records)
        .any(|r| matches!(r, Record::Relation(r) if r.kind == "beam"))
    {
        out.push(Diagnostic::error(
            "beam.legacy_conflict",
            "Authored beam capability cannot be combined with legacy beam relations",
            doc.source,
        ));
    }
    let contexts = native_contexts(doc);
    let events: BTreeMap<_, _> = contexts
        .iter()
        .filter_map(|c| c.id.as_ref().map(|id| (id.as_str(), &c.at)))
        .collect();
    let mut beams = Vec::new();
    for (time, mark) in marks {
        match decode(mark) {
            Ok(beam) => {
                if events
                    .get(beam.event.0.as_str())
                    .is_some_and(|at| *at != time)
                {
                    out.push(error(
                        mark,
                        "beam.onset",
                        "Beam frame must equal the referenced event's exact onset",
                    ));
                }
                beams.push((beam, mark.source));
            }
            Err(diagnostic) => out.push(diagnostic),
        }
    }
    out.extend(validate_contexts(
        &contexts,
        beams.iter().map(|(b, s)| (b, *s)),
    ));
}

fn adjacent(a: &Context, b: &Context) -> bool {
    match (a.order, b.order) {
        (Some(Some(a_order)), Some(Some(b_order))) => {
            a.at == b.at && a_order.checked_add(1) == Some(b_order)
        }
        (None, None) => a
            .duration
            .as_ref()
            .is_some_and(|duration| duration.is_positive() && &a.at + duration == b.at),
        _ => false,
    }
}
fn connected(a: Option<BeamValue>, b: Option<BeamValue>) -> bool {
    matches!(a, Some(BeamValue::Begin | BeamValue::Continue))
        && matches!(b, Some(BeamValue::Continue | BeamValue::End))
}

/// Keep every event in the identity map. Only this topology view may omit the
/// non-head members of a fully checked chord, including an unmarked chord.
/// An invalid group is never hidden just because its head has no beam.
fn chord_topology(
    contexts: &[Context],
    events: &BTreeMap<&str, Vec<usize>>,
    levels: &[[Option<BeamValue>; 8]],
    out: &mut Vec<Diagnostic>,
) -> Vec<bool> {
    let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, context) in contexts.iter().enumerate() {
        if let Some(chord) = &context.chord {
            groups.entry(chord).or_default().push(index);
        }
    }
    let mut omitted = vec![false; contexts.len()];
    for (chord, members) in groups {
        // Collection order is declaration order, never beam-mark or pitch order.
        let head = &contexts[members[0]];
        let beamed = members
            .iter()
            .any(|i| levels[*i].iter().any(Option::is_some));
        let mut problems = Vec::new();
        // Common-model group IDs are opaque and are normalized by adapters.
        // Native identifier syntax is checked separately by the registry.
        if chord.is_empty()
            || members.len() < 2
            || members.windows(2).any(|pair| pair[1] != pair[0] + 1)
            || members.iter().any(|i| {
                let c = &contexts[*i];
                c.lane.is_none()
                    || c.lane != head.lane
                    || c.at != head.at
                    || c.id.as_ref().is_some_and(|id| {
                        events
                            .get(id.as_str())
                            .is_none_or(|indices| indices.len() != 1)
                    })
            })
        {
            problems.push(("beam.chord_group", "A beamed chord requires at least two distinct, consecutive declared members in one actual measure, onset, voice and staff; its group and event identities cannot be reused"));
        }
        if members.iter().any(|i| {
            let c = &contexts[*i];
            c.order.is_some() || !c.pitched || !c.eligible || !c.within_measure
        }) {
            problems.push(("beam.chord_profile", "Beamed chords support only ordinary measured pitched members within their actual measure"));
        }
        if members.iter().any(|i| {
            let c = &contexts[*i];
            c.duration.as_ref().is_none_or(|d| !d.is_positive())
                || c.duration != head.duration
                || c.written.is_none()
                || c.written != head.written
        }) {
            problems.push(("beam.chord_rhythm", "Beamed chord members require equal positive actual durations and equal explicit written type, dot count, time modification, normal notation and graphic size"));
        }
        if !matches!(
            head.stem,
            Some(None | Some(StemDirection::Up | StemDirection::Down))
        ) || members.iter().any(|i| contexts[*i].stem != head.stem)
        {
            problems.push(("beam.chord_stem", "Beamed chord stems must all be absent or all carry the same authored up/down direction"));
        }
        let mut member_beam = false;
        for index in members.iter().skip(1).copied() {
            if levels[index].iter().any(Option::is_some) {
                member_beam = true;
                out.push(contexts[index].error("beam.chord_member", "Only the first declared chord member may carry beams; later members cannot supply or split beam levels"));
            }
        }
        if problems.is_empty() && !member_beam {
            for index in members.iter().skip(1) {
                omitted[*index] = true;
            }
        } else if beamed {
            for (code, message) in problems {
                out.push(head.error(code, message));
            }
        }
    }
    omitted
}

/// One topology checker for both public representations. In particular, a
/// missing beam on an intervening event is not silently skipped.
fn validate_contexts<'a>(
    contexts: &[Context],
    beams: impl Iterator<Item = (&'a ScoreBeam, SourceRange)>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut events: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, c) in contexts.iter().enumerate() {
        if let Some(id) = &c.id {
            events.entry(id).or_default().push(i);
        }
    }
    let mut levels = vec![[None; 8]; contexts.len()];
    for (beam, source) in beams {
        let Some(indices) = events.get(beam.event.0.as_str()).filter(|v| v.len() == 1) else {
            let mut d = Diagnostic::error(
                "beam.event",
                "Beam target must resolve to one owned event",
                source,
            );
            d.object_id = Some(beam.event.0.clone());
            out.push(d);
            continue;
        };
        let i = indices[0];
        let level = beam.number.unwrap_or(1);
        if !(1..=8).contains(&level) {
            out.push(contexts[i].error("beam.number", "Effective beam level must be 1 through 8"));
            continue;
        }
        if levels[i][usize::from(level - 1)]
            .replace(beam.value)
            .is_some()
        {
            out.push(contexts[i].error(
                "beam.duplicate",
                "Each effective beam level may occur only once on an event",
            ));
        }
    }
    let omitted = chord_topology(contexts, &events, &levels, &mut out);
    let mut lanes: BTreeMap<&Lane, Vec<usize>> = BTreeMap::new();
    for (i, context) in contexts.iter().enumerate() {
        if let Some(lane) = context.lane.as_ref().filter(|_| !omitted[i]) {
            lanes.entry(lane).or_default().push(i);
        }
        if levels[i].iter().all(Option::is_none) {
            continue;
        }
        if context.lane.is_none() || !context.within_measure {
            out.push(context.error("beam.measure", "Beamed event must belong to one explicit actual measure without crossing its boundary"));
        }
        if !context.eligible {
            out.push(context.error("beam.profile", "Beams support measured pitched notes/rests, validated measured chord heads and authored before-principal grace notes, without semantic cues"));
        }
        for (level, value) in levels[i]
            .iter()
            .enumerate()
            .filter_map(|(l, v)| v.map(|v| (l, v)))
        {
            if level >= usize::from(context.flags) {
                out.push(context.error("beam.written", "Beam level requires a corresponding explicit written flag (8th through 1024th)"));
            }
            if levels[i][..level].iter().any(Option::is_none) {
                out.push(context.error(
                    "beam.lower_level",
                    "Every secondary beam requires all lower levels on the same event",
                ));
            }
            if level == 0 && matches!(value, BeamValue::ForwardHook | BeamValue::BackwardHook) {
                out.push(context.error(
                    "beam.primary_hook_unsupported",
                    "Primary hooks are outside the authored-beam profile",
                ));
            }
        }
    }
    for (lane, members) in &mut lanes {
        if !members
            .iter()
            .any(|i| levels[*i].iter().any(Option::is_some))
        {
            continue;
        }
        members.sort_by(|a, b| {
            contexts[*a]
                .at
                .cmp(&contexts[*b].at)
                .then_with(|| contexts[*a].order.cmp(&contexts[*b].order))
        });
        if lane.grace_anchor.is_none() {
            // A run cannot choose one of simultaneous or overlapping ordinary
            // events in its voice/staff. Check both directions so an unmarked
            // earlier event cannot disappear merely because no run was open.
            let mut previous_end: Option<Rational> = None;
            let mut previous_beamed_end: Option<Rational> = None;
            for index in members.iter().copied() {
                let c = &contexts[index];
                let beamed = levels[index].iter().any(Option::is_some);
                let conflicts = if beamed {
                    &previous_end
                } else {
                    &previous_beamed_end
                };
                if conflicts.as_ref().is_some_and(|end| end > &c.at) {
                    out.push(c.error("beam.overlap", "Beamed events cannot overlap another event in the same measure, voice and staff"));
                }
                if let Some(duration) = c.duration.as_ref().filter(|d| d.is_positive()) {
                    let end = &c.at + duration;
                    if previous_end.as_ref().is_none_or(|old| old < &end) {
                        previous_end = Some(end.clone());
                    }
                    if beamed && previous_beamed_end.as_ref().is_none_or(|old| old < &end) {
                        previous_beamed_end = Some(end);
                    }
                }
            }
        }
        if let Some(anchor) = &lane.grace_anchor {
            let principal = events
                .get(anchor.as_str())
                .filter(|v| v.len() == 1)
                .map(|v| &contexts[v[0]]);
            let count = i64::try_from(members.len()).ok();
            for (position, index) in members.iter().enumerate() {
                let c = &contexts[*index];
                let expected_order =
                    count.and_then(|n| i64::try_from(position).ok().map(|p| p - n));
                let valid_principal = principal.is_some_and(|p| {
                    p.order.is_none()
                        && p.chord.is_none()
                        && p.within_measure
                        && p.pitched
                        && p.eligible
                        && p.at == c.at
                        && p.lane.as_ref().is_some_and(|l| {
                            l.measure == lane.measure
                                && l.voice == lane.voice
                                && l.staff == lane.staff
                        })
                });
                if c.order != Some(expected_order) || !c.pitched || !c.eligible || !valid_principal
                {
                    out.push(c.error("beam.grace", "Beamed grace groups require an explicit same-onset/voice/staff pitched non-chord principal and consecutive orders -N through -1"));
                }
            }
        }
        for level in 0..8 {
            let mut previous: Option<usize> = None;
            for index in members.iter().copied() {
                let c = &contexts[index];
                match levels[index][level] {
                    Some(BeamValue::Begin) => {
                        if previous.is_some() {
                            out.push(c.error(
                                "beam.sequence",
                                "A new beam begins before the preceding run ends",
                            ));
                        }
                        previous = Some(index);
                    }
                    Some(BeamValue::Continue | BeamValue::End) => {
                        if let Some(prev) = previous {
                            if !adjacent(&contexts[prev], c) {
                                out.push(c.error("beam.adjacency", "Beam members must be immediate, exactly contiguous events in their measure/voice/staff or grace lane"));
                            }
                            if level > 0
                                && !connected(levels[prev][level - 1], levels[index][level - 1])
                            {
                                out.push(c.error("beam.lower_connection", "A secondary beam connection requires the same connection at the next lower level"));
                            }
                        } else {
                            out.push(c.error(
                                "beam.sequence",
                                "Beam continue/end requires an open begin in the same lane",
                            ));
                        }
                        previous =
                            (levels[index][level] == Some(BeamValue::Continue)).then_some(index);
                    }
                    Some(BeamValue::ForwardHook | BeamValue::BackwardHook) | None => {
                        if let Some(prev) = previous {
                            out.push(contexts[prev].error("beam.interrupted", "An open beam run is interrupted by an event without the required continue/end"));
                        }
                        // Hooks are local authored partial-beam orientation, not
                        // connections to an inferred neighboring note or hook.
                        previous = None;
                    }
                }
            }
            if let Some(prev) = previous {
                out.push(contexts[prev].error(
                    "beam.unclosed",
                    "Beam run must end within its actual measure and lane",
                ));
            }
        }
    }
    out
}
