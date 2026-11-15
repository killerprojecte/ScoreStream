//! Required round-trip capability for one initial instrument declaration per part.
#![allow(clippy::result_large_err)]
use crate::{
    Attributes, Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar, SourceRange,
    instrument_metadata::{self as model, InstrumentMetadata, MidiDevice, MidiInstrument},
};
use std::collections::BTreeSet;

pub const EXTENSION: &str = "scorestream.instrument@1";
pub const KIND: &str = "scorestream.instrument";
const FIELDS: &[&str] = &[
    "source_id",
    "name",
    "abbreviation",
    "sound",
    "device",
    "device_ref",
    "port",
    "midi_ref",
    "channel",
    "program",
    "volume",
    "pan",
];
fn error(mark: &Mark, code: &str, message: &str) -> Diagnostic {
    Diagnostic::error(code, message, mark.source)
}
pub fn decode(mark: &Mark) -> Result<InstrumentMetadata, Diagnostic> {
    if mark.kind != KIND
        || mark
            .attributes
            .keys()
            .any(|k| !FIELDS.contains(&k.as_str()))
    {
        return Err(error(
            mark,
            "instrument.field",
            "Unknown instrument metadata kind or field",
        ));
    }
    let string = |key: &str| -> Result<Option<String>, Diagnostic> {
        match mark.attributes.get(key) {
            None => Ok(None),
            Some(Scalar::String(s)) => Ok(Some(s.clone())),
            _ => Err(error(mark, "instrument.field", "Expected instrument text")),
        }
    };
    let required = |key: &str| {
        string(key)?.ok_or_else(|| {
            error(
                mark,
                "instrument.required",
                "Instrument metadata requires source_id and name",
            )
        })
    };
    let number = |key: &str| -> Result<Option<u8>, Diagnostic> {
        match mark.attributes.get(key) {
            None => Ok(None),
            Some(Scalar::Integer(n)) => u8::try_from(*n).map(Some).map_err(|_| {
                error(
                    mark,
                    "instrument.field",
                    "MIDI number outside representable range",
                )
            }),
            _ => Err(error(
                mark,
                "instrument.field",
                "Expected integer MIDI number",
            )),
        }
    };
    let rational = |key: &str| -> Result<Option<Rational>, Diagnostic> {
        string(key)?
            .map(|s| {
                s.parse().map_err(|_| {
                    error(
                        mark,
                        "instrument.field",
                        "Expected canonical exact rational",
                    )
                })
            })
            .transpose()
    };
    let device = string("device")?;
    let midi_ref = string("midi_ref")?;
    if device.is_none()
        && ["device_ref", "port"]
            .iter()
            .any(|k| mark.attributes.contains_key(*k))
        || midi_ref.is_none()
            && ["channel", "program", "volume", "pan"]
                .iter()
                .any(|k| mark.attributes.contains_key(*k))
    {
        return Err(error(
            mark,
            "instrument.assignment",
            "MIDI fields require their explicit device or midi_ref record",
        ));
    }
    let value = InstrumentMetadata {
        source_id: required("source_id")?,
        name: required("name")?,
        abbreviation: string("abbreviation")?,
        sound: string("sound")?,
        midi_device: device
            .map(|name| {
                Ok::<_, Diagnostic>(MidiDevice {
                    name,
                    instrument_ref: string("device_ref")?,
                    port: number("port")?,
                })
            })
            .transpose()?,
        midi_instrument: midi_ref
            .map(|instrument_ref| {
                Ok::<_, Diagnostic>(MidiInstrument {
                    instrument_ref,
                    channel: number("channel")?,
                    program: number("program")?,
                    volume: rational("volume")?,
                    pan: rational("pan")?,
                })
            })
            .transpose()?,
    };
    model::validate(&value).map_err(|e| error(mark, e.code, e.message))?;
    Ok(value)
}
pub fn encode(target: String, value: &InstrumentMetadata) -> Mark {
    let mut attributes = Attributes::new();
    for (key, value) in [
        ("source_id", Some(&value.source_id)),
        ("name", Some(&value.name)),
        ("abbreviation", value.abbreviation.as_ref()),
        ("sound", value.sound.as_ref()),
    ] {
        if let Some(v) = value {
            attributes.insert(key.into(), Scalar::String(v.clone()));
        }
    }
    if let Some(device) = &value.midi_device {
        attributes.insert("device".into(), Scalar::String(device.name.clone()));
        if let Some(id) = &device.instrument_ref {
            attributes.insert("device_ref".into(), Scalar::String(id.clone()));
        }
        if let Some(port) = device.port {
            attributes.insert("port".into(), Scalar::Integer(i64::from(port)));
        }
    }
    if let Some(midi) = &value.midi_instrument {
        attributes.insert(
            "midi_ref".into(),
            Scalar::String(midi.instrument_ref.clone()),
        );
        for (key, value) in [("channel", midi.channel), ("program", midi.program)] {
            if let Some(v) = value {
                attributes.insert(key.into(), Scalar::Integer(i64::from(v)));
            }
        }
        for (key, value) in [("volume", &midi.volume), ("pan", &midi.pan)] {
            if let Some(v) = value {
                attributes.insert(key.into(), Scalar::String(v.to_string()));
            }
        }
    }
    Mark {
        target,
        kind: KIND.into(),
        attributes,
        source: SourceRange::default(),
    }
}
pub(crate) fn validate(doc: &Document, out: &mut Vec<Diagnostic>) {
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let mut parts = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for frame in &doc.frames {
        for record in &frame.records {
            let Record::Mark(mark) = record else { continue };
            if mark.kind != KIND {
                continue;
            }
            if !enabled || doc.document_kind != DocumentKind::Score {
                out.push(error(
                    mark,
                    "instrument.requirement",
                    "Instrument metadata requires scorestream.instrument@1 in a complete score",
                ));
            }
            let part = doc.parts.iter().find(|p| p.id == mark.target);
            if frame.time != Rational::zero()
                || part.is_none()
                || !parts.insert(mark.target.as_str())
            {
                out.push(error(
                    mark,
                    "instrument.scope",
                    "One initial metadata record per declared part at time zero is supported",
                ));
            }
            match decode(mark) {
                Err(e) => out.push(e),
                Ok(value) => {
                    if !sources.insert(value.source_id.clone()) {
                        out.push(error(
                            mark,
                            "instrument.source_id",
                            "Instrument source IDs must be unique",
                        ));
                    }
                    match model::catalog_projection(&value) {
                        Some(projection) if part.is_some_and(|p| p.instrument == projection.canonical_id) => {},
                        Some(_) => out.push(error(mark, "instrument.catalog_mismatch", "Part route must equal the explicit metadata's catalog projection")),
                        None => out.push(error(mark, "instrument.catalog_unresolved", "No exact supported catalog projection; source names or programs are not guessed")),
                    }
                }
            }
        }
    }
    if enabled && parts.is_empty() {
        out.push(Diagnostic::error(
            "instrument.empty_requirement",
            "Instrument capability requires at least one metadata record",
            doc.source,
        ));
    }
}

/// Catalog-only identity metadata does not change existing sound or pitch semantics.
/// Any authored MIDI record, even empty, needs an explicit route realization.
pub fn has_midi_intent(doc: &Document) -> bool {
    doc.frames.iter().flat_map(|f| &f.records).any(|r| matches!(r, Record::Mark(m) if m.kind == KIND && (m.attributes.contains_key("device") || m.attributes.contains_key("midi_ref"))))
}
