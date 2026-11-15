use super::{type_name, xml::escape};
use crate::{Conversion, Format, MappingEffect, MappingReport, validate_score};
use num_bigint::BigInt;
use num_traits::ToPrimitive;
use scorestream_core::{Pitch, Rational, score::*};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
fn issue(r: &mut MappingReport, feature: &str, path: &str, message: impl Into<String>) {
    r.push(
        Format::MusicXml,
        MappingEffect::Unsupported,
        feature,
        path,
        message,
    );
}
pub(super) fn xml_id(s: &str) -> bool {
    s.bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}
fn divisions(
    m: &ScoreMeasure,
    tempo_index: &super::tempo::Export<'_>,
    dynamics_index: &super::dynamics::Export<'_>,
    part: &ScorePart,
) -> Option<u64> {
    let mut n = 1;
    let offsets = tempo_index.positions(part, m);
    let dynamics_offsets = dynamics_index.offsets(part, m);
    for value in std::iter::once(&m.length)
        .chain(m.events.iter().flat_map(|e| [&e.offset, &e.duration]))
        .chain(offsets.iter())
        .chain(dynamics_offsets.iter())
    {
        let d = value.denominator().to_u64()?;
        n = (n / gcd(n, d)).checked_mul(d)?;
        if n > 1_000_000_000 {
            return None;
        }
    }
    Some(n)
}
fn units(value: &Rational, div: u64) -> String {
    (value * &Rational::from_big_integer(div.into()))
        .numerator()
        .to_string()
}
pub(super) fn decimal(value: &Rational) -> Option<String> {
    let mut d = value.denominator().clone();
    let mut twos = 0u32;
    let mut fives = 0u32;
    while &d % 2u8 == BigInt::from(0) {
        d /= 2;
        twos += 1;
        if twos > 32 {
            return None;
        }
    }
    while &d % 5u8 == BigInt::from(0) {
        d /= 5;
        fives += 1;
        if fives > 32 {
            return None;
        }
    }
    if d != BigInt::from(1) {
        return None;
    }
    let scale = twos.max(fives);
    let n =
        value.numerator() * BigInt::from(2).pow(scale - twos) * BigInt::from(5).pow(scale - fives);
    if scale == 0 {
        return Some(n.to_string());
    }
    let text = n.to_string();
    let digits = text.trim_start_matches('-');
    let digits = format!("{:0>width$}", digits, width = scale as usize + 1);
    let at = digits.len() - scale as usize;
    Some(format!(
        "{}{}.{}",
        if n < BigInt::from(0) { "-" } else { "" },
        &digits[..at],
        &digits[at..]
    ))
}
pub fn write(score: &ScoreDocument) -> Result<Conversion<String>, MappingReport> {
    write_with_options(score, &super::MusicXmlOptions::default())
}

/// Export an explicitly authorized irregular bar without inventing a time signature
/// or changing numbering. Reimporting its XML requires the same exact contract.
pub fn write_with_options(
    score: &ScoreDocument,
    options: &super::MusicXmlOptions,
) -> Result<Conversion<String>, MappingReport> {
    options.validate()?;
    let mut report = validate_score(score);
    if report.has_errors() {
        return Err(report);
    }
    super::extent::validate_export(score, options, &mut report);
    super::tempo::validate_export(score, &mut report);
    let tempo_index = super::tempo::Export::new(score);
    let dynamics_index = super::dynamics::Export::new(score, &mut report);
    let fermatas = super::fermatas::Export::new(score, &mut report);
    for (field, value) in score.metadata.text_fields() {
        if value.is_some()
            && ["description", "header", "footer", "subtitle", "language"].contains(&field)
        {
            issue(
                &mut report,
                &format!("metadata.{field}"),
                "/metadata",
                "No lossless MusicXML mapping is implemented for this field",
            );
        }
    }
    for value in score
        .metadata
        .text_fields()
        .into_iter()
        .filter_map(|(_, v)| v)
        .chain(score.metadata.authors.iter().map(String::as_str))
        .chain(score.parts.iter().flat_map(|p| {
            p.name
                .iter()
                .chain(p.instrument.iter().map(|v| &v.0))
                .map(String::as_str)
        }))
        .chain(
            score
                .parts
                .iter()
                .flat_map(|p| p.measures.iter().map(|m| m.label.as_str())),
        )
    {
        if value.chars().any(|c| !super::xml::valid_char(c)) {
            issue(
                &mut report,
                "xml.character",
                "/",
                "Text contains a character forbidden by XML 1.0; it is not removed",
            );
        }
    }
    let mut xml_ids = BTreeSet::new();
    for part in &score.parts {
        if !xml_id(&part.id.0) {
            issue(
                &mut report,
                "part.id",
                &part.id.0,
                "Part ID is outside the supported XML ID profile",
            );
        }
        if !xml_ids.insert(part.id.0.clone()) {
            issue(&mut report, "xml.id", &part.id.0, "Duplicate XML ID");
        }
        for voice in &part.voices {
            if voice.id.0.chars().any(|c| !super::xml::valid_char(c)) {
                issue(
                    &mut report,
                    "voice.id",
                    &part.id.0,
                    "Voice identity contains an invalid XML character",
                );
            }
        }
        let mut meters: BTreeMap<u32, Meter> = BTreeMap::new();
        for (measure_index, m) in part.measures.iter().enumerate() {
            if !xml_id(&m.id) || !xml_ids.insert(m.id.clone()) {
                issue(
                    &mut report,
                    "measure.id",
                    &m.id,
                    "Invalid or duplicate XML ID",
                );
            }
            if divisions(m, &tempo_index, &dynamics_index, part).is_none() {
                issue(
                    &mut report,
                    "divisions.limit",
                    &m.id,
                    "Exact division LCM exceeds 1000000000; duration is not rounded",
                );
            }
            for s in &m.states {
                if let Some(meter) = &s.meter {
                    meters.insert(s.staff, meter.clone());
                }
                if s.transpose
                    .as_ref()
                    .is_some_and(|t| decimal(&t.chromatic).is_none_or(|value| value.len() > 64))
                {
                    issue(
                        &mut report,
                        "transpose.chromatic",
                        &m.id,
                        "Chromatic transposition needs an exact finite decimal of at most 32 places and 64 characters",
                    );
                }
            }
            if !m.implicit
                && options.at(measure_index + 1).is_none()
                && meters.values().any(|meter| meter.quarters() != m.length)
            {
                issue(
                    &mut report,
                    "measure.length",
                    &m.id,
                    "Non-implicit measure length must equal its active time signature",
                );
            }
            let mut chords: BTreeMap<&str, &ScoreEvent> = BTreeMap::new();
            for e in &m.events {
                if !xml_id(&e.id.0) || !xml_ids.insert(e.id.0.clone()) {
                    issue(
                        &mut report,
                        "event.id",
                        &e.id.0,
                        "Invalid or duplicate XML ID",
                    );
                }
                match &e.pitch {
                    Pitch::Rest => {}
                    Pitch::Pitched {
                        octave, alteration, ..
                    } => {
                        if !octave.to_i64().is_some_and(|n| (0..=9).contains(&n)) {
                            issue(
                                &mut report,
                                "pitch.octave",
                                &e.id.0,
                                "MusicXML octave must be 0 through 9",
                            );
                        }
                        if decimal(alteration).is_none() {
                            issue(
                                &mut report,
                                "pitch.alter",
                                &e.id.0,
                                "Alteration needs an exact finite decimal of at most 32 places",
                            );
                        }
                    }
                    _ => issue(
                        &mut report,
                        "pitch.kind",
                        &e.id.0,
                        "Unpitched notes and spacers are outside this profile",
                    ),
                }
                if let Some(c) = &e.chord {
                    if let Some(head) = chords.get(c.as_str()) {
                        if e.duration > head.duration {
                            issue(
                                &mut report,
                                "chord.duration",
                                &e.id.0,
                                "Chord tone exceeds its first tone; reordering the head is not implicit",
                            );
                        }
                    } else {
                        chords.insert(c, e);
                        report.push(Format::MusicXml, MappingEffect::Normalized, "chord.identity", c, "MusicXML stores ordered chord membership, not a chord-group ID; group identity is regenerated on reimport");
                    }
                }
            }
            // Detect overlaps in a voice rather than flattening polyphony into chords.
            for v in &part.voices {
                let mut events: Vec<_> = m.events.iter().filter(|e| e.voice == v.id).collect();
                events.sort_by(|a, b| a.offset.cmp(&b.offset));
                // The first measure always emits an event or a voice/staff forward.
                // Later cross-staff notes cannot change that declared default.
                if measure_index == 0
                    && events
                        .first()
                        .is_some_and(|e| e.offset == Rational::zero() && e.staff != v.staff)
                {
                    issue(
                        &mut report,
                        "voice.default_staff",
                        &v.id.0,
                        "MusicXML has no voice default-staff declaration; first note on a different staff cannot preserve this authoring default",
                    );
                }
                let mut cursor = Rational::zero();
                let mut active_chord = None;
                for e in events {
                    if e.chord.is_some() && e.chord.as_ref() == active_chord {
                        continue;
                    }
                    if e.offset < cursor {
                        issue(
                            &mut report,
                            "voice.overlap",
                            &e.id.0,
                            "Overlapping events in one voice need explicit chord membership or separate voices",
                        );
                    }
                    cursor = &e.offset + &e.duration;
                    active_chord = e.chord.as_ref();
                }
            }
        }
    }
    let barlines = super::barlines::output(score, &mut xml_ids, &mut report);
    // Partwise measure positions are shared. Reject ambiguous independent part timelines.
    let first = &score.parts[0];
    for p in score.parts.iter().skip(1) {
        if p.measures.len() != first.measures.len()
            || p.measures
                .iter()
                .zip(&first.measures)
                .any(|(a, b)| a.start != b.start || a.length != b.length)
        {
            issue(
                &mut report,
                "part.measure_alignment",
                &p.id.0,
                "This profile requires aligned partwise measure boundaries",
            );
        }
    }
    if report.has_errors() {
        return Err(report);
    }
    let tie_endpoints = super::ties::output(score, &mut report)?;
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<score-partwise version=\"4.0\">\n",
    );
    if score.metadata.title.is_some() || score.metadata.work_number.is_some() {
        out.push_str("  <work>");
        if let Some(number) = &score.metadata.work_number {
            write!(out, "<work-number>{}</work-number>", escape(number)).unwrap();
        }
        if let Some(title) = &score.metadata.title {
            write!(out, "<work-title>{}</work-title>", escape(title)).unwrap();
        }
        out.push_str("</work>\n");
    }
    if let Some(movement) = &score.metadata.movement {
        writeln!(
            out,
            "  <movement-title>{}</movement-title>",
            escape(movement)
        )
        .unwrap();
    }
    if score.metadata.composer.is_some()
        || score.metadata.lyricist.is_some()
        || score.metadata.arranger.is_some()
        || !score.metadata.authors.is_empty()
        || score.metadata.rights.is_some()
    {
        out.push_str("  <identification>\n");
        for (role, value) in [
            ("composer", &score.metadata.composer),
            ("lyricist", &score.metadata.lyricist),
            ("arranger", &score.metadata.arranger),
        ] {
            if let Some(value) = value {
                writeln!(
                    out,
                    "    <creator type=\"{role}\">{}</creator>",
                    escape(value)
                )
                .unwrap();
            }
        }
        for author in &score.metadata.authors {
            writeln!(
                out,
                "    <creator type=\"author\">{}</creator>",
                escape(author)
            )
            .unwrap();
        }
        if let Some(rights) = &score.metadata.rights {
            writeln!(out, "    <rights>{}</rights>", escape(rights)).unwrap();
        }
        out.push_str("  </identification>\n");
    }
    out.push_str("  <part-list>\n");
    for (i, p) in score.parts.iter().enumerate() {
        let name = p
            .name
            .as_deref()
            .unwrap_or_else(|| p.instrument.as_ref().map(|v| v.0.as_str()).unwrap_or(""));
        writeln!(
            out,
            "    <score-part id=\"{}\"><part-name>{}</part-name>",
            escape(&p.id.0),
            escape(name)
        )
        .unwrap();
        if p.name.is_none() {
            report.push(
                Format::MusicXml,
                MappingEffect::Normalized,
                "part.name",
                &p.id.0,
                "Required part-name derived from instrument label or left empty",
            );
        }
        if let Some(instrument) = &p.instrument {
            let mut id = format!("mx_instrument{}", i + 1);
            while xml_ids.contains(&id) {
                id.push('_');
            }
            xml_ids.insert(id.clone());
            writeln!(out, "      <score-instrument id=\"{id}\"><instrument-name>{}</instrument-name></score-instrument>", escape(&instrument.0)).unwrap();
        }
        out.push_str("    </score-part>\n");
    }
    out.push_str("  </part-list>\n");
    for p in &score.parts {
        writeln!(out, "  <part id=\"{}\">", escape(&p.id.0)).unwrap();
        for (mi, m) in p.measures.iter().enumerate() {
            let div = divisions(m, &tempo_index, &dynamics_index, p).unwrap();
            writeln!(
                out,
                "    <measure id=\"{}\" number=\"{}\" implicit=\"{}\">",
                escape(&m.id),
                escape(&m.label),
                if m.implicit { "yes" } else { "no" }
            )
            .unwrap();
            barlines.emit(&mut out, &m.id, BarlineLocation::Left);
            writeln!(out, "      <attributes><divisions>{div}</divisions>").unwrap();
            for s in &m.states {
                if let Some(key) = s.key {
                    writeln!(
                        out,
                        "        <key number=\"{}\"><fifths>{}</fifths></key>",
                        s.staff, key.fifths
                    )
                    .unwrap();
                }
            }
            for s in &m.states {
                if let Some(meter) = &s.meter {
                    writeln!(out, "        <time number=\"{}\"><beats>{}</beats><beat-type>{}</beat-type></time>", s.staff, meter.beats.iter().map(ToString::to_string).collect::<Vec<_>>().join("+"), meter.beat_type).unwrap();
                }
            }
            if mi == 0 {
                writeln!(out, "        <staves>{}</staves>", p.staves).unwrap();
            }
            for s in &m.states {
                if let Some(clef) = s.clef {
                    writeln!(
                        out,
                        "        <clef number=\"{}\"><sign>{}</sign><line>{}</line></clef>",
                        s.staff,
                        match clef.sign {
                            ClefSign::G => "G",
                            ClefSign::F => "F",
                            ClefSign::C => "C",
                        },
                        clef.line
                    )
                    .unwrap();
                }
            }
            for s in &m.states {
                if let Some(t) = &s.transpose {
                    writeln!(out, "        <transpose number=\"{}\">", s.staff).unwrap();
                    if let Some(diatonic) = t.diatonic {
                        writeln!(out, "          <diatonic>{diatonic}</diatonic>").unwrap();
                    }
                    writeln!(
                        out,
                        "          <chromatic>{}</chromatic>",
                        decimal(&t.chromatic).unwrap()
                    )
                    .unwrap();
                    writeln!(
                        out,
                        "          <octave-change>{}</octave-change>",
                        t.octaves
                    )
                    .unwrap();
                    out.push_str("        </transpose>\n");
                }
            }
            out.push_str("      </attributes>\n");
            super::tempo::write_measure(&tempo_index, p, m, div, &mut out);
            dynamics_index.write_measure(p, m, div, &mut out);
            for (vi, v) in p.voices.iter().enumerate() {
                if vi > 0 {
                    writeln!(
                        out,
                        "      <backup><duration>{}</duration></backup>",
                        units(&m.length, div)
                    )
                    .unwrap();
                }
                let mut events: Vec<_> = m.events.iter().filter(|e| e.voice == v.id).collect();
                events.sort_by(|a, b| a.offset.cmp(&b.offset));
                let mut cursor = Rational::zero();
                let mut active_chord = None;
                for e in events {
                    let is_chord = e.chord.is_some() && active_chord == e.chord.as_ref();
                    if !is_chord && cursor < e.offset {
                        writeln!(out, "      <forward><duration>{}</duration><voice>{}</voice><staff>{}</staff></forward>", units(&(&e.offset - &cursor), div), escape(&v.id.0), v.staff).unwrap();
                    }
                    writeln!(out, "      <note id=\"{}\">", escape(&e.id.0)).unwrap();
                    if is_chord {
                        out.push_str("        <chord/>\n");
                    }
                    match &e.pitch {
                        Pitch::Rest => out.push_str("        <rest/>\n"),
                        Pitch::Pitched { step, alteration, octave } => writeln!(out, "        <pitch><step>{step}</step><alter>{}</alter><octave>{octave}</octave></pitch>", decimal(alteration).unwrap()).unwrap(),
                        _ => unreachable!("validated"),
                    }
                    writeln!(
                        out,
                        "        <duration>{}</duration>",
                        units(&e.duration, div)
                    )
                    .unwrap();
                    let tied = tie_endpoints.get(&e.id);
                    if tied.is_some_and(|t| t.stop.is_some()) {
                        out.push_str("        <tie type=\"stop\"/>\n");
                    }
                    if tied.is_some_and(|t| t.start.is_some()) {
                        out.push_str("        <tie type=\"start\"/>\n");
                    }
                    writeln!(out, "        <voice>{}</voice>", escape(&v.id.0)).unwrap();
                    if let Some(w) = &e.written {
                        let size = w
                            .size
                            .map(|s| format!(" size=\"{}\"", s.name()))
                            .unwrap_or_default();
                        writeln!(out, "        <type{size}>{}</type>", type_name(w.note_type))
                            .unwrap();
                        for _ in 0..w.dots {
                            out.push_str("        <dot/>\n");
                        }
                        if let Some(tm) = &w.time_modification {
                            writeln!(out, "        <time-modification><actual-notes>{}</actual-notes><normal-notes>{}</normal-notes>", tm.actual, tm.normal).unwrap();
                            if let Some(t) = tm.normal_type {
                                writeln!(
                                    out,
                                    "          <normal-type>{}</normal-type>",
                                    type_name(t)
                                )
                                .unwrap();
                            }
                            for _ in 0..tm.normal_dots {
                                out.push_str("          <normal-dot/>\n");
                            }
                            out.push_str("        </time-modification>\n");
                        }
                    }
                    writeln!(out, "        <staff>{}</staff>", e.staff).unwrap();
                    if tied.is_some() || fermatas.contains(&e.id) {
                        out.push_str("        <notations>");
                        if let Some(n) = tied.and_then(|t| t.stop) {
                            write!(out, "<tied type=\"stop\" number=\"{n}\"/>").unwrap();
                        }
                        if let Some(n) = tied.and_then(|t| t.start) {
                            write!(out, "<tied type=\"start\" number=\"{n}\"/>").unwrap();
                        }
                        fermatas.write(&e.id, &mut out);
                        out.push_str("</notations>\n");
                    }
                    out.push_str("      </note>\n");
                    if !is_chord {
                        cursor = &e.offset + &e.duration;
                    }
                    active_chord = e.chord.as_ref();
                }
                if cursor < m.length {
                    writeln!(out, "      <forward><duration>{}</duration><voice>{}</voice><staff>{}</staff></forward>", units(&(&m.length - &cursor), div), escape(&v.id.0), v.staff).unwrap();
                }
            }
            barlines.emit(&mut out, &m.id, BarlineLocation::Right);
            out.push_str("    </measure>\n");
        }
        out.push_str("  </part>\n");
    }
    out.push_str("</score-partwise>\n");
    report.finish(out)
}
