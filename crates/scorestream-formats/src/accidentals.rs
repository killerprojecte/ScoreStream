//! Printed glyphs are independent authored metadata, never pitch edits.
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{
    Attributes, Document, Mark, Pitch, Rational, Record, Scalar, SourceRange, score::*,
};
use std::collections::BTreeMap;

pub(crate) fn validate(score: &ScoreDocument, report: &mut MappingReport) {
    let measured = score
        .parts
        .iter()
        .flat_map(|p| &p.measures)
        .flat_map(|m| &m.events)
        .filter_map(|e| e.accidental.as_ref().map(|a| (&e.id, &e.pitch, a)));
    let graces = score
        .graces
        .iter()
        .filter_map(|g| g.accidental.as_ref().map(|a| (&g.id, &g.pitch, a)));
    for (id, pitch, accidental) in measured.chain(graces) {
        let Pitch::Pitched { alteration, .. } = pitch else {
            report.push(
                Format::ScoreModel,
                MappingEffect::Invalid,
                "accidental.target",
                &id.0,
                "Printed accidental requires a pitched measured or grace note",
            );
            continue;
        };
        // Keep the existing SFS consistency boundary. Never modify the source pitch
        // to agree with a glyph, nor derive a glyph from pitch or key signatures.
        let expected = match accidental.glyph {
            AccidentalGlyph::Natural => "0",
            AccidentalGlyph::Sharp => "1",
            AccidentalGlyph::Flat => "-1",
            AccidentalGlyph::DoubleSharp => "2",
            AccidentalGlyph::FlatFlat => "-2",
            AccidentalGlyph::QuarterSharp => "1/2",
            AccidentalGlyph::QuarterFlat => "-1/2",
        };
        if *alteration != expected.parse::<Rational>().unwrap() {
            report.push(Format::ScoreModel,MappingEffect::Unsupported,"accidental.pitch_profile",&id.0,
                "Authored glyph conflicts with written pitch within the existing SFS accidental profile; neither is rewritten");
        }
    }
}

pub(crate) fn read_marks(
    doc: &Document,
    report: &mut MappingReport,
) -> BTreeMap<String, PrintedAccidental> {
    let mut result = BTreeMap::new();
    for record in doc.frames.iter().flat_map(|f| &f.records) {
        let Record::Mark(m) = record else { continue };
        if m.kind != "accidental" {
            continue;
        }
        for key in m.attributes.keys() {
            if !["value", "cautionary", "parentheses"].contains(&key.as_str()) {
                report.push_source(Format::Sfs,MappingEffect::Unsupported,"accidental.attribute",&m.target,
                    format!("Accidental attribute {key} has no exact MusicXML/common mapping in this bounded profile"),Some(m.source));
            }
        }
        let Some(glyph) = m
            .attributes
            .get("value")
            .and_then(Scalar::as_str)
            .and_then(AccidentalGlyph::from_name)
        else {
            continue;
        };
        let Some(event) = m.target.strip_prefix('#') else {
            continue;
        };
        let a = PrintedAccidental {
            glyph,
            cautionary: m.attributes.get("cautionary").and_then(Scalar::as_bool),
            parentheses: m.attributes.get("parentheses").and_then(Scalar::as_bool),
        };
        if result.insert(event.into(), a).is_some() {
            report.push_source(Format::Sfs,MappingEffect::Unsupported,"accidental.duplicate",&m.target,
                "Only one authored accidental per note is represented; duplicate marks are not coalesced",Some(m.source));
        }
    }
    result
}
pub(crate) fn write_mark(
    accidental: Option<&PrintedAccidental>,
    event: &str,
    at: Rational,
    frames: &mut BTreeMap<Rational, Vec<Record>>,
) {
    let Some(a) = accidental else { return };
    let mut attributes = Attributes::new();
    attributes.insert("value".into(), Scalar::String(a.glyph.name().into()));
    for (key, value) in [("cautionary", a.cautionary), ("parentheses", a.parentheses)] {
        if let Some(value) = value {
            attributes.insert(key.into(), Scalar::Boolean(value));
        }
    }
    frames.entry(at).or_default().push(Record::Mark(Mark {
        target: format!("#{event}"),
        kind: "accidental".into(),
        attributes,
        source: SourceRange::default(),
    }));
}
