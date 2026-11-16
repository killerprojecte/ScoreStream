//! Ordinary sound + notation tie pairing, without carrying XML into the score.
use super::xml::Node;
use crate::{Format, MappingEffect, MappingReport, ties::event_index};
use scorestream_core::{
    Rational, SourceRange,
    notation::{EventId, PartId, RelationId, VoiceId},
    score::*,
};
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, MappingReport>;
#[derive(Clone)]
pub(super) struct Endpoints {
    start: Option<u8>,
    stop: Option<u8>,
    source: SourceRange,
    path: String,
}
impl Endpoints {
    fn error(&self, feature: &str, message: &str) -> MappingReport {
        let mut report = MappingReport::default();
        report.push_source(
            Format::MusicXml,
            MappingEffect::Unsupported,
            feature,
            &self.path,
            message,
            Some(self.source),
        );
        report
    }
}
pub(super) fn read_endpoints(note: &Node) -> Result<Option<Endpoints>> {
    read_endpoints_with_empty(note, false)
}
pub(super) fn read_endpoints_with_empty(
    note: &Node,
    allow_empty: bool,
) -> Result<Option<Endpoints>> {
    let mut result = Endpoints {
        start: None,
        stop: None,
        source: note.source,
        path: note.path.clone(),
    };
    let mut sound_start = false;
    let mut sound_stop = false;
    for tie in note.children_named("tie") {
        if tie.attr("time-only").is_some() {
            return Err(result.error(
                "tie.repeat_condition",
                "Repeat-conditioned ties require a repeat-aware common model",
            ));
        }
        let target = match tie.attr("type") {
            Some("start") => &mut sound_start,
            Some("stop") => &mut sound_stop,
            _ => return Err(result.error("tie.type", "Sound tie type must be start or stop")),
        };
        if *target {
            return Err(result.error("tie.duplicate", "Duplicate sound tie endpoint on a note"));
        }
        *target = true;
    }
    let mut seen_start = false;
    for notation in note.children_named("notations") {
        if notation.children.is_empty() && !allow_empty {
            return Err(result.error(
                "tie.empty_notations",
                "Empty notations have no mapped meaning",
            ));
        }
        for tied in notation.children_named("tied") {
            let number = tied
                .attr("number")
                .unwrap_or("1")
                .parse::<u8>()
                .ok()
                .filter(|n| (1..=16).contains(n))
                .ok_or_else(|| result.error("tie.number", "Tie number must be 1 through 16"))?;
            let target = match tied.attr("type") {
                Some("start") => { seen_start = true; &mut result.start },
                Some("stop") if !seen_start => &mut result.stop,
                Some("stop") => return Err(result.error("tie.repeat_shape", "Start then stop on one note can mark a repeat/coda boundary; it is not a measured continuation")),
                _ => return Err(result.error("tie.notation_type", "Only ordinary start/stop notation ties are mapped; let-ring and layout continue are distinct semantics")),
            };
            if target.replace(number).is_some() {
                return Err(result.error("tie.duplicate", "Multiple notation endpoints with the same phase cannot be paired with one sound endpoint"));
            }
        }
    }
    if sound_start != result.start.is_some() || sound_stop != result.stop.is_some() {
        return Err(result.error("tie.sound_notation_mismatch", "This profile requires matching sound and notation endpoints; print-only and sound-only ties are not guessed"));
    }
    Ok((sound_start || sound_stop).then_some(result))
}

type BoundaryKey<'a> = (&'a PartId, &'a VoiceId, Rational, Rational, u8);
#[derive(Default)]
struct Boundary<'a> {
    starts: Vec<&'a EventId>,
    stops: Vec<&'a EventId>,
}
pub(super) fn resolve(
    score: &ScoreDocument,
    endpoints: BTreeMap<EventId, Endpoints>,
    report: &mut MappingReport,
) -> Result<Vec<ScoreTie>> {
    if endpoints.is_empty() {
        return Ok(vec![]);
    }
    let index = event_index(score);
    let mut boundaries = BTreeMap::<BoundaryKey<'_>, Boundary<'_>>::new();
    for (id, endpoint) in &endpoints {
        let current = &index[id];
        let pitch = current
            .pitch
            .clone()
            .ok_or_else(|| endpoint.error("tie.pitch", "Ties require pitched notes"))?;
        if let Some(number) = endpoint.start {
            boundaries
                .entry((
                    current.part,
                    &current.event.voice,
                    pitch.clone(),
                    current.end.clone(),
                    number,
                ))
                .or_default()
                .starts
                .push(id);
        }
        if let Some(number) = endpoint.stop {
            boundaries
                .entry((
                    current.part,
                    &current.event.voice,
                    pitch,
                    current.start.clone(),
                    number,
                ))
                .or_default()
                .stops
                .push(id);
        }
    }
    // Resolve an entire boundary, never consume a source in destination-ID order.
    // Two destinations competing for one same-staff source are ambiguous even if
    // greedy matching could assign the remaining source across a staff boundary.
    for boundary in boundaries.values() {
        if !boundary.stops.is_empty() && boundary.starts.len() != boundary.stops.len() {
            return Err(endpoints[boundary.stops[0]].error("tie.pairing", "Tie stops do not have the same number of contiguous starts in this part/voice/effective pitch and local number"));
        }
    }
    let mut links = Vec::new();
    for boundary in boundaries.into_values() {
        if boundary.stops.is_empty() {
            return Err(endpoints[boundary.starts[0]].error("tie.unclosed", "Tie start has no matching contiguous stop; ties to repeats or missing notes are not guessed"));
        }
        let mut staffs = BTreeMap::<u32, Boundary<'_>>::new();
        for id in boundary.starts {
            staffs
                .entry(index[id].event.staff)
                .or_default()
                .starts
                .push(id);
        }
        for id in boundary.stops {
            staffs
                .entry(index[id].event.staff)
                .or_default()
                .stops
                .push(id);
        }
        let mut remaining = Boundary::default();
        for staff in staffs.into_values() {
            match (staff.starts.as_slice(), staff.stops.as_slice()) {
                ([start], [stop]) => links.push((*start, *stop)),
                ([], stops) => remaining.stops.extend_from_slice(stops),
                (starts, []) => remaining.starts.extend_from_slice(starts),
                (_, stops) => return Err(endpoints[stops[0]].error(
                    "tie.pairing",
                    "Multiple same-staff sources or destinations make unison tie pairing ambiguous",
                )),
            }
        }
        match (remaining.starts.as_slice(), remaining.stops.as_slice()) {
            ([], []) => {}
            ([start], [stop]) => links.push((*start, *stop)),
            (_, stops) => {
                return Err(endpoints[stops[0]].error(
                    "tie.pairing",
                    "Multiple residual cross-staff unisons cannot be paired uniquely",
                ));
            }
        }
    }
    links.sort_by(|(_, a), (_, b)| {
        index[a]
            .start
            .cmp(&index[b].start)
            .then(index[a].part.cmp(index[b].part))
            .then(a.cmp(b))
    });
    let mut ties: Vec<_> = links
        .into_iter()
        .enumerate()
        .map(|(i, (start, stop))| ScoreTie {
            id: RelationId(format!("mx_t{}", i + 1)),
            events: vec![start.clone(), stop.clone()],
        })
        .collect();
    ties.sort_by(|a, b| a.id.cmp(&b.id));
    report.push(Format::MusicXml, MappingEffect::Normalized, "tie.identity", "/", "Part-local tie numbers are resolved into stable relation/event identities; XML numbering is regenerated on export");
    Ok(ties)
}

#[derive(Default)]
pub(super) struct OutputEndpoints {
    pub start: Option<u8>,
    pub stop: Option<u8>,
}
/// Reuse a local number only after a preceding tie has ended. Numbering is scoped
/// by part/voice/effective pitch and is never used as a global notation identity.
pub(super) fn output<'a>(
    score: &'a ScoreDocument,
    report: &mut MappingReport,
) -> Result<BTreeMap<&'a EventId, OutputEndpoints>> {
    if score.ties.is_empty() {
        return Ok(BTreeMap::new());
    }
    let index = event_index(score);
    let mut spans: Vec<_> = score
        .ties
        .iter()
        .flat_map(|t| t.events.windows(2))
        .collect();
    spans.sort_by(|a, b| {
        index[&a[0]]
            .start
            .cmp(&index[&b[0]].start)
            .then(a[0].cmp(&b[0]))
    });
    let mut lanes = BTreeMap::new();
    let mut output: BTreeMap<&EventId, OutputEndpoints> = BTreeMap::new();
    for pair in spans {
        let a = &index[&pair[0]];
        let b = &index[&pair[1]];
        let ends = lanes
            .entry((a.part, &a.event.voice, a.pitch.clone()))
            .or_insert_with(|| vec![Rational::zero(); 16]);
        let Some(slot) = ends.iter().position(|end| *end <= a.start) else {
            let mut error = MappingReport::default();
            error.push(
                Format::MusicXml,
                MappingEffect::Unsupported,
                "tie.number_capacity",
                &pair[0].0,
                "More than 16 simultaneous same-pitch ties in a voice cannot be numbered exactly",
            );
            return Err(error);
        };
        ends[slot] = b.start.clone();
        let number = (slot + 1) as u8;
        output.entry(&pair[0]).or_default().start = Some(number);
        output.entry(&pair[1]).or_default().stop = Some(number);
    }
    if !output.is_empty() {
        report.push(Format::MusicXml, MappingEffect::Normalized, "tie.identity", "/", "MusicXML has local numbered endpoints, not SFS relation identities; chain IDs are regenerated on import while note IDs and links are preserved");
    }
    Ok(output)
}
