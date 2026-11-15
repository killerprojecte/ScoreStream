//! Authored single-instrument identity and initial MIDI intent, not a sound engine.
//! Source identities, labels and assignments remain distinct from the catalog key.
use crate::{Rational, instruments, notation::InstrumentId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentMetadata {
    /// Original document-scoped score-instrument ID, never a MIDI program or part ID.
    pub source_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abbreviation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi_device: Option<MidiDevice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi_instrument: Option<MidiInstrument>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiDevice {
    /// Empty is an authored empty device element, not an absent element.
    pub name: String,
    /// None affects the part's sole instrument; an explicit IDREF is retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrument_ref: Option<String>,
    /// Source notation is one-based. None must not become port 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiInstrument {
    pub instrument_ref: String,
    /// Source notation is one-based, 1..=16. Not a backend channel allocation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<u8>,
    /// Source notation is one-based, 1..=128. No GM bank is implied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<u8>,
    /// Exact 0..=100 percent channel-volume intent, distinct from note dynamics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<Rational>,
    /// Exact -180..=180 degrees, not an already quantized MIDI CC10 value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pan: Option<Rational>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetadataError {
    pub code: &'static str,
    pub message: &'static str,
}
fn error(code: &'static str, message: &'static str) -> MetadataError {
    MetadataError { code, message }
}

/// Current importer/exporter supports this exact ASCII subset of XML IDs.
pub fn source_id_valid(id: &str) -> bool {
    id.bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
pub fn text_valid(text: &str) -> bool {
    text.chars().all(|c| matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'))
}
pub fn validate(value: &InstrumentMetadata) -> Result<(), MetadataError> {
    if !source_id_valid(&value.source_id) {
        return Err(error(
            "instrument.source_id",
            "Instrument source ID is outside the supported XML ID profile",
        ));
    }
    for text in std::iter::once(&value.name)
        .chain(&value.abbreviation)
        .chain(&value.sound)
        .chain(value.midi_device.iter().map(|v| &v.name))
    {
        if !text_valid(text) {
            return Err(error(
                "instrument.text",
                "Instrument text contains an XML-forbidden character",
            ));
        }
    }
    if let Some(device) = &value.midi_device {
        if device
            .instrument_ref
            .as_ref()
            .is_some_and(|id| id != &value.source_id)
        {
            return Err(error(
                "instrument.idref",
                "Device IDREF must refer to the part's sole score-instrument",
            ));
        }
        if device.port.is_some_and(|n| !(1..=16).contains(&n)) {
            return Err(error(
                "instrument.midi_port",
                "MIDI port must be 1 through 16 when present",
            ));
        }
    }
    if let Some(midi) = &value.midi_instrument {
        if midi.instrument_ref != value.source_id {
            return Err(error(
                "instrument.idref",
                "MIDI IDREF must refer to the part's sole score-instrument",
            ));
        }
        if midi.channel.is_some_and(|n| !(1..=16).contains(&n)) {
            return Err(error(
                "instrument.midi_channel",
                "MIDI channel must be 1 through 16 when present",
            ));
        }
        if midi.program.is_some_and(|n| !(1..=128).contains(&n)) {
            return Err(error(
                "instrument.midi_program",
                "MIDI program must be 1 through 128 when present",
            ));
        }
        if midi
            .volume
            .as_ref()
            .is_some_and(|n| n.is_negative() || n > &Rational::from_integer(100))
        {
            return Err(error(
                "instrument.midi_volume",
                "MIDI volume must be an exact percentage from 0 through 100",
            ));
        }
        if midi
            .pan
            .as_ref()
            .is_some_and(|n| n < &Rational::from_integer(-180) || n > &Rational::from_integer(180))
        {
            return Err(error(
                "instrument.midi_pan",
                "Pan must be exact degrees from -180 through 180",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogProjection {
    pub canonical_id: &'static str,
    /// The source subtype is retained in metadata, but has a broader catalog key.
    pub broader: bool,
}

/// Finite, reviewed MusicXML 4.0 sound identities. No prefix matching, language
/// guessing, MIDI-program inference or pitch/transposition inference is allowed.
pub fn catalog_projection(value: &InstrumentMetadata) -> Option<CatalogProjection> {
    if let Some(sound) = &value.sound {
        let (canonical_id, broader) = match sound.as_str() {
            "wind.flutes.flute" => ("flute", false),
            "wind.flutes.flute.piccolo" => ("piccolo", false),
            "wind.reed.oboe" => ("oboe", false),
            "wind.reed.clarinet" => ("clarinet", false),
            "wind.reed.clarinet.bflat" => ("clarinet", true),
            "wind.reed.bassoon" => ("bassoon", false),
            "wind.reed.contrabassoon" => ("contrabassoon", false),
            "brass.natural-horn" => ("horn", true),
            "brass.trumpet.c" => ("trumpet", true),
            "brass.trombone.alto" => ("alto_trombone", false),
            "brass.trombone.tenor" => ("trombone", true),
            "brass.trombone.bass" => ("bass_trombone", false),
            "drum.timpani" => ("timpani", false),
            "strings.violin" => ("violin", false),
            "strings.viola" => ("viola", false),
            "strings.cello" => ("cello", false),
            "strings.contrabass" => ("double_bass", false),
            _ => return None,
        };
        return Some(CatalogProjection {
            canonical_id,
            broader,
        });
    }
    // An absent sound may use an exact catalog ID/alias or exact catalog label.
    // An explicit but unknown sound never falls through to this name path.
    let by_id = instruments::resolve_instrument(&InstrumentId(value.name.clone()));
    let definition = by_id.or_else(|| {
        instruments::instrument_catalog()
            .iter()
            .find(|d| d.name == value.name)
    })?;
    Some(CatalogProjection {
        canonical_id: &definition.id.0,
        broader: false,
    })
}
