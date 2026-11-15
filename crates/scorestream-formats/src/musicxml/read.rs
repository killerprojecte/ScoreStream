use super::{
    note_type,
    xml::{self, Node},
};
use crate::{Conversion, MappingEffect, MappingReport, validate_score};
use num_bigint::BigInt;
use scorestream_core::{
    Pitch, Rational,
    notation::{EventId, InstrumentId, PartId, VoiceId},
    score::*,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, Cursor, Seek, SeekFrom},
};

type Result<T> = std::result::Result<T, MappingReport>;
fn error(n: &Node, feature: &str, message: impl Into<String>) -> MappingReport {
    let mut r = MappingReport::default();
    n.error(&mut r, MappingEffect::Invalid, feature, message);
    r
}
fn unsupported(n: &Node, feature: &str, message: impl Into<String>) -> MappingReport {
    let mut r = MappingReport::default();
    n.error(&mut r, MappingEffect::Unsupported, feature, message);
    r
}
fn required_attr<'a>(n: &'a Node, key: &str) -> Result<&'a str> {
    n.attr(key)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| error(n, "required.attribute", format!("Missing {key}")))
}
/// XML decimal lexical space to exact reduced rational, never via floating point.
fn decimal(n: &Node) -> Result<Rational> {
    let value = n.text.trim();
    if value.len() > 64 {
        return Err(unsupported(
            n,
            "number.precision",
            "Numeric literals are limited to 64 characters",
        ));
    }
    let signed = value.strip_prefix(['+', '-']).unwrap_or(value);
    let (whole, fraction) = signed.split_once('.').unwrap_or((signed, ""));
    if (whole.is_empty() && fraction.is_empty())
        || !whole.bytes().all(|b| b.is_ascii_digit())
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(error(n, "number.decimal", "Expected an XML decimal"));
    }
    let digits = format!("{whole}{fraction}")
        .parse::<BigInt>()
        .map_err(|_| error(n, "number.decimal", "Invalid decimal"))?;
    let num = Rational::from_big_integer(if value.starts_with('-') {
        -digits
    } else {
        digits
    });
    Ok(&num / &Rational::from_big_integer(BigInt::from(10).pow(fraction.len() as u32)))
}
fn positive(n: &Node) -> Result<Rational> {
    let value = decimal(n)?;
    if value.is_positive() {
        Ok(value)
    } else {
        Err(error(n, "number.positive", "Expected a positive number"))
    }
}
fn integer(n: &Node) -> Result<i64> {
    n.text
        .trim()
        .parse()
        .map_err(|_| error(n, "number.integer", "Expected a bounded integer"))
}
fn uint(n: &Node) -> Result<u32> {
    integer(n)
        .ok()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0)
        .ok_or_else(|| error(n, "number.positive_integer", "Expected positive integer"))
}
fn child<'a>(n: &'a Node, name: &str) -> Result<&'a Node> {
    n.child(name)
        .ok_or_else(|| error(n, "required.element", format!("Missing {name}")))
}
fn bool_attr(n: &Node, key: &str) -> Result<bool> {
    match n.attr(key) {
        None | Some("no") => Ok(false),
        Some("yes") => Ok(true),
        _ => Err(error(n, "boolean", format!("{key} must be yes or no"))),
    }
}
fn staff_number(n: &Node, staves: u32, attribute: bool) -> Result<u32> {
    let value = if attribute {
        n.attr("number")
    } else {
        n.value("staff")
    };
    let staff = value
        .unwrap_or("1")
        .parse::<u32>()
        .map_err(|_| error(n, "staff.number", "Invalid staff number"))?;
    if staff == 0 || staff > staves {
        Err(error(
            n,
            "staff.number",
            "Staff number outside declared staves",
        ))
    } else {
        Ok(staff)
    }
}
fn voice(
    n: &Node,
    part_index: usize,
    map: &mut BTreeMap<String, VoiceId>,
    voices: &mut Vec<ScoreVoice>,
    staff: u32,
    report: &mut MappingReport,
) -> Result<VoiceId> {
    let label = n.value("voice").unwrap_or("1");
    if label.is_empty() {
        return Err(error(n, "voice.label", "Empty voice label"));
    }
    if let Some(v) = map.get(label) {
        return Ok(v.clone());
    }
    let id = VoiceId(format!("v{}_{}", part_index + 1, map.len() + 1));
    if label != id.0 {
        n.error(
            report,
            MappingEffect::Normalized,
            "voice.identity",
            format!("Part-local voice {label:?} assigned shared identity {id}"),
        );
    }
    map.insert(label.into(), id.clone());
    voices.push(ScoreVoice {
        id: id.clone(),
        staff,
    });
    Ok(id)
}
/// String convenience API; files should use `read_reader` to avoid buffering their XML.
pub fn read(source: &str) -> Result<Conversion<ScoreDocument>> {
    read_reader(Cursor::new(source.as_bytes()))
}

/// Two incremental passes preserve forward ID references and declaration order without a DOM.
/// Pass one validates each released fragment and indexes IDs; pass two maps one measure at a time.
/// The shared score and its identity index necessarily scale with the musical content.
pub fn read_reader<R: BufRead + Seek>(mut input: R) -> Result<Conversion<ScoreDocument>> {
    let beginning = input.stream_position().map_err(io_error)?;
    let mut headers = Vec::new();
    let mut root = None;
    let mut part_nodes = BTreeMap::new();
    let mut part_note_counts = BTreeMap::<String, usize>::new();
    let mut object_ids = BTreeSet::new();
    let mut report = MappingReport::default();
    let first_pass = xml::walk(&mut input, |node, parent, phase| {
        if phase == xml::Phase::PartStart {
            return Ok(());
        }
        xml::schema(node, &mut report);
        if let Err(errors) = reserve_ids(node, &mut object_ids) {
            report.append(errors);
        }
        match parent.map(|p| p.name.as_str()) {
            Some("score-partwise") => {
                if node.name == "part" {
                    let id = required_attr(node, "id")?.to_string();
                    if part_nodes.insert(id, node.clone()).is_some() {
                        return Err(error(
                            node,
                            "part.reference",
                            "Expected exactly one matching part",
                        ));
                    }
                } else if ["work", "movement-title", "identification", "part-list"]
                    .contains(&node.name.as_str())
                {
                    if !headers.iter().any(|n: &Node| n.name == node.name) {
                        headers.push(node.clone());
                    }
                } else {
                    node.error(
                        &mut report,
                        MappingEffect::Unsupported,
                        &format!("element.{}", node.name),
                        "Element has no implemented mapping at this path",
                    );
                }
            }
            Some("part") => {
                if node.name != "measure" {
                    node.error(
                        &mut report,
                        MappingEffect::Unsupported,
                        &format!("element.{}", node.name),
                        "Only measures are mapped within a part",
                    );
                }
                let id = required_attr(parent.unwrap(), "id")?.to_string();
                *part_note_counts.entry(id).or_default() += node.children_named("note").count();
            }
            None => root = Some(node.clone()),
            _ => {}
        }
        Ok(())
    })?;
    if report.has_errors() {
        return Err(report);
    }
    let mut root = root.expect("walk requires a root");
    root.children = headers;
    if root.name != "score-partwise" {
        return Err(unsupported(
            &root,
            "root",
            "Only score-partwise is supported",
        ));
    }
    if root.attr("version") != Some("4.0") {
        return Err(unsupported(
            &root,
            "musicxml.version",
            "This strict profile requires explicit MusicXML version 4.0",
        ));
    }
    let mut score = ScoreDocument::default();
    if let Some(work) = root.child("work") {
        score.metadata.title = work.child("work-title").map(|n| n.text.clone());
        score.metadata.work_number = work.child("work-number").map(|n| n.text.clone());
    }
    score.metadata.movement = root.child("movement-title").map(|n| n.text.clone());
    if let Some(identification) = root.child("identification") {
        score.metadata.rights = identification.child("rights").map(|n| n.text.clone());
        for creator in identification.children_named("creator") {
            let target = match creator.attr("type") {
                Some("author") => {
                    score.metadata.authors.push(creator.text.clone());
                    continue;
                }
                Some("composer") => &mut score.metadata.composer,
                Some("lyricist") => &mut score.metadata.lyricist,
                Some("arranger") => &mut score.metadata.arranger,
                _ => {
                    return Err(unsupported(
                        creator,
                        "metadata.creator_role",
                        "Creator role is not represented; unspecified roles are not guessed to be authors",
                    ));
                }
            };
            if target.is_some() {
                return Err(unsupported(
                    creator,
                    "metadata.multiple_creators",
                    "Multiple creators in this role need a richer metadata model",
                ));
            }
            *target = Some(creator.text.clone());
        }
    }
    let declarations = child(&root, "part-list")?;
    let mut indices = BTreeMap::new();
    let mut starts = Vec::new();
    let mut note_count = 0usize;
    for (part_index, declared) in declarations.children_named("score-part").enumerate() {
        let id = required_attr(declared, "id")?;
        if indices.insert(id.to_string(), part_index).is_some() {
            return Err(error(declared, "part.identity", "Duplicate score-part ID"));
        }
        if !part_nodes.contains_key(id) {
            return Err(error(
                declared,
                "part.reference",
                "Expected exactly one matching part",
            ));
        }
        starts.push(note_count);
        note_count += part_note_counts.get(id).copied().unwrap_or(0);
    }
    for (id, part) in &part_nodes {
        if !indices.contains_key(id) {
            return Err(error(
                part,
                "part.reference",
                "Part ID not declared by part-list",
            ));
        }
    }
    let declared: Vec<_> = declarations.children_named("score-part").collect();
    let mut parts: Vec<Option<ScorePart>> = vec![None; declared.len()];
    let mut active = None;
    let mut tie_endpoints = BTreeMap::new();
    input.seek(SeekFrom::Start(beginning)).map_err(io_error)?;
    let second_pass = xml::walk(&mut input, |node, parent, phase| {
        // A seekable source can change between passes. Never rely on prior validation
        // for an unchecked match or index in the semantic mapper.
        if phase == xml::Phase::Complete {
            let mut checked = MappingReport::default();
            xml::schema(node, &mut checked);
            if checked.has_errors() {
                return Err(checked);
            }
        }
        if phase == xml::Phase::PartStart {
            let id = required_attr(node, "id")?;
            let part_index = *indices.get(id).ok_or_else(|| {
                error(node, "part.reference", "Part ID not declared by part-list")
            })?;
            active = Some(PartReader::new(
                declared[part_index],
                part_index,
                starts[part_index],
                &mut report,
            )?);
        } else if parent.is_some_and(|p| p.name == "part") {
            if node.name != "measure" {
                return Err(unsupported(
                    node,
                    "part.content",
                    "Only measures are mapped within a part",
                ));
            }
            active
                .as_mut()
                .ok_or_else(|| error(node, "part.reference", "Measure outside an active part"))?
                .measure(node, &mut object_ids, &mut report)?;
        } else if node.name == "part" && parent.is_some_and(|p| p.name == "score-partwise") {
            let mut reader = active
                .take()
                .ok_or_else(|| error(node, "part.reference", "Missing active part"))?;
            if parts[reader.part_index].is_some() {
                return Err(error(
                    node,
                    "part.reference",
                    "Part changed or repeated between passes",
                ));
            }
            if reader.part.voices.is_empty() {
                reader.part.voices.push(ScoreVoice {
                    id: VoiceId(format!("v{}_1", reader.part_index + 1)),
                    staff: 1,
                });
            }
            tie_endpoints.append(&mut reader.tie_endpoints);
            parts[reader.part_index] = Some(reader.part);
        }
        Ok(())
    })?;
    if first_pass.fingerprint != second_pass.fingerprint {
        return Err(error(
            &root,
            "xml.changed",
            "Input changed between validation and mapping; retry with a stable source",
        ));
    }
    score.parts = parts
        .into_iter()
        .map(|part| {
            part.ok_or_else(|| error(&root, "part.reference", "Part disappeared between passes"))
        })
        .collect::<Result<_>>()?;
    if let Some(first) = score.parts.first() {
        for part in score.parts.iter().skip(1) {
            if part.measures.len() != first.measures.len()
                || part
                    .measures
                    .iter()
                    .zip(&first.measures)
                    .any(|(a, b)| a.start != b.start || a.length != b.length)
            {
                return Err(unsupported(
                    &root,
                    "part.measure_alignment",
                    "Unequal or ambiguous partwise boundaries need a later profile; no part timeline is guessed",
                ));
            }
        }
    }
    score.ties = super::ties::resolve(&score, tie_endpoints, &mut report)?;
    report.append(validate_score(&score));
    report.finish(score)
}
fn io_error(error: std::io::Error) -> MappingReport {
    xml::fail("xml.io", error.to_string(), MappingEffect::Invalid)
}

struct PartReader {
    part_index: usize,
    part: ScorePart,
    divisions: Option<Rational>,
    meters: BTreeMap<u32, Meter>,
    voice_map: BTreeMap<String, VoiceId>,
    start: Rational,
    note_counter: usize,
    tie_endpoints: BTreeMap<EventId, super::ties::Endpoints>,
}
impl PartReader {
    fn new(
        declared: &Node,
        part_index: usize,
        note_counter: usize,
        report: &mut MappingReport,
    ) -> Result<Self> {
        let id = required_attr(declared, "id")?;
        let instrument = declared
            .child("score-instrument")
            .map(|n| {
                let original = required_attr(n, "id")?;
                n.error(report, MappingEffect::Normalized, "instrument.identity", format!("XML instrument reference {original:?} is replaced by its instrument-name in the shared profile; no external instrument references are accepted"));
                Ok(InstrumentId(child(n, "instrument-name")?.text.clone()))
            })
            .transpose()?;
        let part = ScorePart {
            id: PartId(id.into()),
            name: Some(child(declared, "part-name")?.text.clone()),
            instrument,
            staves: 1,
            voices: vec![],
            measures: vec![],
        };
        Ok(Self {
            part_index,
            part,
            divisions: None,
            meters: BTreeMap::new(),
            voice_map: BTreeMap::new(),
            start: Rational::zero(),
            note_counter,
            tie_endpoints: BTreeMap::new(),
        })
    }
    fn measure(
        &mut self,
        measure: &Node,
        object_ids: &mut BTreeSet<String>,
        report: &mut MappingReport,
    ) -> Result<()> {
        let Self {
            part_index,
            part,
            divisions,
            meters,
            voice_map,
            start,
            note_counter,
            tie_endpoints,
        } = self;
        let part_index = *part_index;
        let mi = part.measures.len();
        let label = required_attr(measure, "number")?.to_string();
        let implicit = bool_attr(measure, "implicit")?;
        let mut m = ScoreMeasure {
            id: measure
                .attr("id")
                .map(String::from)
                .unwrap_or_else(|| fresh_id(format!("m{}_{}", part_index + 1, mi + 1), object_ids)),
            label,
            start: start.clone(),
            length: Rational::zero(),
            implicit,
            states: vec![],
            events: vec![],
        };
        let mut cursor = Rational::zero();
        let mut extent = Rational::zero();
        let mut moved = false;
        let mut attributes_seen = false;
        let mut previous: Option<usize> = None;
        for n in &measure.children {
            match n.name.as_str() {
                "attributes" => {
                    previous = None;
                    if moved {
                        return Err(unsupported(
                            n,
                            "attributes.mid_measure",
                            "Only measure-start attributes are supported; document-order overrides are not guessed",
                        ));
                    }
                    if attributes_seen {
                        return Err(unsupported(
                            n,
                            "attributes.multiple",
                            "Multiple attributes blocks at one boundary require a later profile",
                        ));
                    }
                    attributes_seen = true;
                    if let Some(d) = n.child("divisions") {
                        *divisions = Some(positive(d)?);
                    }
                    if let Some(s) = n.child("staves") {
                        let count = uint(s)?;
                        if count > 64 {
                            return Err(unsupported(
                                s,
                                "staves.limit",
                                "At most 64 staves per part",
                            ));
                        }
                        if mi != 0 && count != part.staves {
                            return Err(unsupported(
                                s,
                                "staves.change",
                                "Changing staff count is not supported",
                            ));
                        }
                        part.staves = count;
                    }
                    for state in n
                        .children
                        .iter()
                        .filter(|s| ["key", "time", "clef", "transpose"].contains(&s.name.as_str()))
                    {
                        let targets = if state.attr("number").is_none() && state.name != "clef" {
                            (1..=part.staves).collect::<Vec<_>>()
                        } else {
                            vec![staff_number(state, part.staves, true)?]
                        };
                        for staff in targets {
                            if !m.states.iter().any(|s| s.staff == staff) {
                                m.states.push(StaffState {
                                    staff,
                                    clef: None,
                                    key: None,
                                    meter: None,
                                    transpose: None,
                                });
                            }
                            let target = m.states.iter_mut().find(|s| s.staff == staff).unwrap();
                            match state.name.as_str() {
                                "key" => {
                                    let value = KeySignature {
                                        fifths: integer(child(state, "fifths")?)?,
                                    };
                                    if target.key.is_some() {
                                        return Err(error(
                                            state,
                                            "attributes.conflict",
                                            "Repeated key for staff",
                                        ));
                                    }
                                    target.key = Some(value);
                                }
                                "time" => {
                                    let text = child(state, "beats")?.text.trim();
                                    let beats = text
                                        .split('+')
                                        .map(|s| s.parse::<u32>().ok().filter(|n| *n > 0))
                                        .collect::<Option<Vec<_>>>()
                                        .ok_or_else(|| {
                                            error(
                                                state,
                                                "time.beats",
                                                "Expected positive beats or additive beats",
                                            )
                                        })?;
                                    let meter = Meter {
                                        beats,
                                        beat_type: uint(child(state, "beat-type")?)?,
                                    };
                                    if target.meter.is_some() {
                                        return Err(error(
                                            state,
                                            "attributes.conflict",
                                            "Repeated time for staff",
                                        ));
                                    }
                                    meters.insert(staff, meter.clone());
                                    target.meter = Some(meter);
                                }
                                "transpose" => {
                                    if target.transpose.is_some() {
                                        return Err(error(
                                            state,
                                            "attributes.conflict",
                                            "Repeated transposition for staff",
                                        ));
                                    }
                                    target.transpose = Some(Transposition {
                                        chromatic: decimal(child(state, "chromatic")?)?,
                                        diatonic: state
                                            .child("diatonic")
                                            .map(integer)
                                            .transpose()?,
                                        octaves: state
                                            .child("octave-change")
                                            .map(integer)
                                            .transpose()?
                                            .unwrap_or(0),
                                    });
                                }
                                "clef" => {
                                    let sign = match state.value("sign") {
                                        Some("G") => ClefSign::G,
                                        Some("F") => ClefSign::F,
                                        Some("C") => ClefSign::C,
                                        _ => {
                                            return Err(unsupported(
                                                state,
                                                "clef.sign",
                                                "Only G, F and C clefs are mapped",
                                            ));
                                        }
                                    };
                                    let line = u8::try_from(uint(child(state, "line")?)?).map_err(
                                        |_| error(state, "clef.line", "Invalid clef line"),
                                    )?;
                                    if target.clef.is_some() {
                                        return Err(error(
                                            state,
                                            "attributes.conflict",
                                            "Repeated clef for staff",
                                        ));
                                    }
                                    target.clef = Some(Clef { sign, line });
                                }
                                _ => unreachable!(),
                            }
                        }
                    }
                }
                "backup" | "forward" => {
                    previous = None;
                    moved = true;
                    let div = divisions.as_ref().ok_or_else(|| {
                        error(
                            n,
                            "divisions.missing",
                            "Duration requires explicit divisions",
                        )
                    })?;
                    let delta = &positive(child(n, "duration")?)? / div;
                    if n.name == "backup" {
                        cursor = &cursor - &delta;
                        if cursor.is_negative() {
                            return Err(error(
                                n,
                                "backup.boundary",
                                "Backup crosses measure start",
                            ));
                        }
                    } else {
                        cursor = &cursor + &delta;
                        if extent < cursor {
                            extent = cursor.clone();
                        }
                        let staff = staff_number(n, part.staves, false)?;
                        if n.child("voice").is_some() {
                            voice(n, part_index, voice_map, &mut part.voices, staff, report)?;
                        }
                    }
                }
                "note" => {
                    moved = true;
                    let div = divisions.as_ref().ok_or_else(|| {
                        error(
                            n,
                            "divisions.missing",
                            "Note duration requires explicit divisions",
                        )
                    })?;
                    let duration = &positive(child(n, "duration")?)? / div;
                    let staff = staff_number(n, part.staves, false)?;
                    let v = voice(n, part_index, voice_map, &mut part.voices, staff, report)?;
                    if n.child("pitch").is_some() == n.child("rest").is_some() {
                        return Err(error(
                            n,
                            "note.pitch",
                            "Exactly one pitch or rest is required",
                        ));
                    }
                    let pitch = if let Some(p) = n.child("pitch") {
                        let step = match p.value("step") {
                            Some("A") => 'A',
                            Some("B") => 'B',
                            Some("C") => 'C',
                            Some("D") => 'D',
                            Some("E") => 'E',
                            Some("F") => 'F',
                            Some("G") => 'G',
                            _ => return Err(error(p, "pitch.step", "Expected A through G")),
                        };
                        let octave = integer(child(p, "octave")?)?;
                        if !(0..=9).contains(&octave) {
                            return Err(error(
                                p,
                                "pitch.octave",
                                "MusicXML octave must be 0 through 9",
                            ));
                        }
                        Pitch::Pitched {
                            step,
                            alteration: p
                                .child("alter")
                                .map(decimal)
                                .transpose()?
                                .unwrap_or_default(),
                            octave: octave.into(),
                        }
                    } else {
                        Pitch::Rest
                    };
                    let written = if let Some(t) = n.value("type") {
                        let note_type = note_type(t).ok_or_else(|| {
                            unsupported(n, "note.type", "Unmapped written note type")
                        })?;
                        let time_modification = n
                            .child("time-modification")
                            .map(|tm| {
                                let normal_type = tm
                                    .value("normal-type")
                                    .map(|v| note_type_parse(tm, v))
                                    .transpose()?;
                                let normal_dots = tm.children_named("normal-dot").count() as u32;
                                if normal_dots > 0 && normal_type.is_none() {
                                    return Err(error(
                                        tm,
                                        "time_modification.normal_type",
                                        "Normal dots require normal type",
                                    ));
                                }
                                Ok(TimeModification {
                                    actual: uint(child(tm, "actual-notes")?)?,
                                    normal: uint(child(tm, "normal-notes")?)?,
                                    normal_type,
                                    normal_dots,
                                })
                            })
                            .transpose()?;
                        Some(WrittenDuration {
                            note_type,
                            dots: n.children_named("dot").count() as u32,
                            time_modification,
                        })
                    } else {
                        if n.child("dot").is_some() || n.child("time-modification").is_some() {
                            return Err(unsupported(
                                n,
                                "note.written_duration",
                                "Dots or time modification without explicit type cannot be represented",
                            ));
                        }
                        None
                    };
                    let mut offset = cursor.clone();
                    let mut chord = None;
                    if n.child("chord").is_some() {
                        let index = previous.ok_or_else(|| {
                            error(
                                n,
                                "chord.anchor",
                                "Chord tone requires an immediately preceding note",
                            )
                        })?;
                        let prior = &mut m.events[index];
                        if prior.voice != v || prior.pitch == Pitch::Rest || pitch == Pitch::Rest {
                            return Err(error(
                                n,
                                "chord.voice",
                                "Chord must contain pitched notes in the same voice",
                            ));
                        }
                        if duration > prior.duration {
                            return Err(error(
                                n,
                                "chord.duration",
                                "Chord tone cannot outlast its first note",
                            ));
                        }
                        offset = prior.offset.clone();
                        let group = prior
                            .chord
                            .get_or_insert_with(|| format!("c{}_{}", part_index + 1, note_counter));
                        chord = Some(group.clone());
                    } else {
                        cursor = &cursor + &duration;
                        if extent < cursor {
                            extent = cursor.clone();
                        }
                        previous = Some(m.events.len());
                    }
                    *note_counter += 1;
                    let id = n
                        .attr("id")
                        .map(String::from)
                        .unwrap_or_else(|| fresh_id(format!("mx_n{note_counter}"), object_ids));
                    if let Some(endpoints) = super::ties::read_endpoints(n)? {
                        tie_endpoints.insert(EventId(id.clone()), endpoints);
                    }
                    m.events.push(ScoreEvent {
                        id: EventId(id),
                        voice: v,
                        staff,
                        offset,
                        duration,
                        pitch,
                        chord,
                        written,
                    });
                }
                _ => {
                    return Err(unsupported(
                        n,
                        "measure.content",
                        "Element has no implemented mapping within a measure",
                    ));
                }
            }
        }
        let nominal = meters.values().next().map(Meter::quarters);
        if nominal
            .as_ref()
            .is_some_and(|first| meters.values().any(|m| m.quarters() != *first))
        {
            return Err(unsupported(
                measure,
                "meter.polymeter",
                "Unequal staff meter lengths require a later profile",
            ));
        }
        m.length = if implicit {
            extent.clone()
        } else {
            nominal.unwrap_or_else(|| extent.clone())
        };
        if extent > m.length {
            return Err(error(
                measure,
                "measure.overfull",
                "Notes or forward extend beyond the meter boundary",
            ));
        }
        if !m.length.is_positive() {
            return Err(error(
                measure,
                "measure.empty",
                "Cannot infer a positive length for an empty measure without meter",
            ));
        }
        *start = &*start + &m.length;
        part.measures.push(m);
        Ok(())
    }
}

fn note_type_parse(n: &Node, s: &str) -> Result<NoteType> {
    note_type(s).ok_or_else(|| unsupported(n, "note.type", "Unsupported normal note type"))
}

fn reserve_ids(n: &Node, ids: &mut BTreeSet<String>) -> Result<()> {
    if let Some(id) = n.attr("id").filter(|_| n.name != "part") {
        if !super::write::xml_id(id) {
            return Err(unsupported(
                n,
                "xml.id",
                "ID is outside the supported ASCII XML ID profile",
            ));
        }
        if !ids.insert(id.to_string()) {
            return Err(error(n, "xml.id", "Duplicate XML ID"));
        }
    }
    for child in &n.children {
        reserve_ids(child, ids)?;
    }
    Ok(())
}
fn fresh_id(mut proposed: String, ids: &mut BTreeSet<String>) -> String {
    while !ids.insert(proposed.clone()) {
        proposed.push('_');
    }
    proposed
}
