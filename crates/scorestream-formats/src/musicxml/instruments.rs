//! Bounded initial single-instrument identity, separate from device realization.
use super::{
    read::{decimal, error, unsupported},
    xml::{Node, escape},
};
use crate::{MappingEffect, MappingReport};
use scorestream_core::{
    instrument_metadata::{self as model, InstrumentMetadata, MidiDevice, MidiInstrument},
    notation::InstrumentId,
};
use std::{collections::BTreeSet, fmt::Write};
type Result<T> = std::result::Result<T, MappingReport>;
fn required(n: &Node, key: &str) -> Result<String> {
    n.attr(key)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| error(n, "instrument.required", format!("Missing {key}")))
}
fn number(n: &Node, value: &str) -> Result<u8> {
    value
        .trim()
        .parse()
        .map_err(|_| error(n, "instrument.integer", "Expected a bounded integer"))
}
pub(super) fn read(
    n: &Node,
    report: &mut MappingReport,
) -> Result<Option<(InstrumentId, InstrumentMetadata)>> {
    let Some(declaration) = n.child("score-instrument") else {
        if n.child("midi-device").is_some() || n.child("midi-instrument").is_some() {
            return Err(unsupported(
                n,
                "instrument.missing_declaration",
                "MIDI intent requires a single score-instrument declaration",
            ));
        }
        return Ok(None);
    };
    let name = declaration
        .child("instrument-name")
        .ok_or_else(|| error(declaration, "instrument.name", "Missing instrument name"))?
        .text
        .clone();
    let midi_device = n
        .child("midi-device")
        .map(|d| -> Result<_> {
            Ok(MidiDevice {
                name: d.text.clone(),
                instrument_ref: d.attr("id").map(str::to_owned),
                port: d.attr("port").map(|v| number(d, v)).transpose()?,
            })
        })
        .transpose()?;
    let midi_instrument = n
        .child("midi-instrument")
        .map(|m| -> Result<_> {
            Ok(MidiInstrument {
                instrument_ref: required(m, "id")?,
                channel: m
                    .child("midi-channel")
                    .map(|v| number(v, &v.text))
                    .transpose()?,
                program: m
                    .child("midi-program")
                    .map(|v| number(v, &v.text))
                    .transpose()?,
                volume: m.child("volume").map(decimal).transpose()?,
                pan: m.child("pan").map(decimal).transpose()?,
            })
        })
        .transpose()?;
    let value = InstrumentMetadata {
        source_id: required(declaration, "id")?,
        name,
        abbreviation: declaration
            .child("instrument-abbreviation")
            .map(|v| v.text.clone()),
        sound: declaration
            .child("instrument-sound")
            .map(|v| v.text.clone()),
        midi_device,
        midi_instrument,
    };
    model::validate(&value).map_err(|e| error(declaration, e.code, e.message))?;
    let projection = model::catalog_projection(&value).ok_or_else(|| unsupported(declaration, "instrument.catalog_unresolved", "No exact supported catalog projection; unknown sound/name is retained in the error context, never guessed from MIDI program"))?;
    if projection.broader || value.name != projection.canonical_id {
        declaration.error(report, MappingEffect::Normalized, "instrument.catalog_projection", format!("Catalog identity is {}; exact authored ID, label, sound and MIDI intent are retained separately{}", projection.canonical_id, if projection.broader { "; the catalog identity is broader than the source subtype" } else { "" }));
    }
    if value.midi_device.is_some() || value.midi_instrument.is_some() {
        declaration.error(report, MappingEffect::Normalized, "instrument.midi_authored_only", "Initial MIDI intent is preserved exactly; playback requires explicit route realization and currently rejects this capability's MIDI payload");
    }
    Ok(Some((InstrumentId(projection.canonical_id.into()), value)))
}
pub(super) fn validate_export(
    value: &InstrumentMetadata,
    ids: &mut BTreeSet<String>,
    report: &mut MappingReport,
) {
    if !ids.insert(value.source_id.clone()) {
        report.push(
            crate::Format::MusicXml,
            MappingEffect::Invalid,
            "xml.id",
            &value.source_id,
            "Instrument source ID collides with another XML identity",
        );
    }
    if let Some(midi) = &value.midi_instrument {
        for n in [&midi.volume, &midi.pan].into_iter().flatten() {
            if super::write::decimal(n).is_none() {
                report.push(crate::Format::MusicXml, MappingEffect::Unsupported, "instrument.decimal", &value.source_id, "MIDI percent/angle cannot be exported as a bounded exact decimal; it is not rounded");
            }
        }
    }
}
pub(super) fn write(out: &mut String, value: &InstrumentMetadata) {
    writeln!(
        out,
        "      <score-instrument id=\"{}\">",
        escape(&value.source_id)
    )
    .unwrap();
    writeln!(
        out,
        "        <instrument-name>{}</instrument-name>",
        escape(&value.name)
    )
    .unwrap();
    for (tag, text) in [
        ("instrument-abbreviation", &value.abbreviation),
        ("instrument-sound", &value.sound),
    ] {
        if let Some(text) = text {
            writeln!(out, "        <{tag}>{}</{tag}>", escape(text)).unwrap();
        }
    }
    out.push_str("      </score-instrument>\n");
    if let Some(d) = &value.midi_device {
        out.push_str("      <midi-device");
        if let Some(port) = d.port {
            write!(out, " port=\"{port}\"").unwrap();
        }
        if let Some(id) = &d.instrument_ref {
            write!(out, " id=\"{}\"", escape(id)).unwrap();
        }
        writeln!(out, ">{}</midi-device>", escape(&d.name)).unwrap();
    }
    if let Some(m) = &value.midi_instrument {
        writeln!(
            out,
            "      <midi-instrument id=\"{}\">",
            escape(&m.instrument_ref)
        )
        .unwrap();
        for (tag, n) in [("midi-channel", m.channel), ("midi-program", m.program)] {
            if let Some(n) = n {
                writeln!(out, "        <{tag}>{n}</{tag}>").unwrap();
            }
        }
        for (tag, n) in [("volume", &m.volume), ("pan", &m.pan)] {
            if let Some(n) = n {
                writeln!(
                    out,
                    "        <{tag}>{}</{tag}>",
                    super::write::decimal(n).expect("validated exact MIDI decimal")
                )
                .unwrap();
            }
        }
        out.push_str("      </midi-instrument>\n");
    }
}

#[cfg(test)]
mod tests {
    use super::super::xml;
    use super::*;
    use std::{
        collections::BTreeSet,
        fs::File,
        io::{BufReader, Cursor},
        path::PathBuf,
    };
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to the pinned original XML/MXL directory"]
    fn original_beethoven_77_instrument_declarations_preserve_raw_metadata_and_routes() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let mut total = 0;
        let mut names_with_spaces = 0;
        let mut sounds = BTreeSet::new();
        let mut ports = [0; 4];
        for movement in 1..=4 {
            let mut count = 0;
            let path = root.join(format!("Beethoven_Op.67_{movement}.musicxml"));
            let scan = xml::walk(BufReader::new(File::open(&path).unwrap()), |node, _, phase| {
                if phase != xml::Phase::Complete || node.name != "part-list" { return Ok(()); }
                for part in node.children_named("score-part") {
                    assert_eq!(part.children_named("score-instrument").count(), 1);
                    assert_eq!(part.children_named("midi-device").count(), 1);
                    assert_eq!(part.children_named("midi-instrument").count(), 1);
                    for name in ["score-instrument", "midi-device", "midi-instrument"] {
                        let mut report = MappingReport::default();
                        xml::schema(part.child(name).unwrap(), &mut report);
                        assert!(!report.has_errors(), "{report:?}");
                    }
                    let (identity, value) = read(part, &mut MappingReport::default())?.unwrap();
                    assert_eq!(value.source_id, format!("{}-I1", part.attr("id").unwrap()));
                    let device = value.midi_device.as_ref().unwrap();
                    assert_eq!(device.name, "");
                    assert_eq!(device.instrument_ref.as_ref(), Some(&value.source_id));
                    ports[device.port.unwrap_or(0) as usize] += 1;
                    let midi = value.midi_instrument.as_ref().unwrap();
                    assert_eq!(midi.volume.as_ref().unwrap().to_string(), "393701/5000");
                    assert_eq!(midi.pan.as_ref().unwrap().to_string(), "0");
                    assert_eq!(midi.instrument_ref, value.source_id);
                    assert_eq!(identity.0, model::catalog_projection(&value).unwrap().canonical_id);
                    names_with_spaces += usize::from(value.name.contains(' '));
                    sounds.insert(value.sound.clone().unwrap());
                    let mark = scorestream_core::instrument_extension::encode("p".into(), &value);
                    assert_eq!(scorestream_core::instrument_extension::decode(&mark).unwrap(), value);
                    // Only declaration metadata is round-tripped here. Original measures,
                    // labels, unsupported engraving and performance remain untouched.
                    let mut fragment = String::from("<score-partwise><part-list><score-part id=\"p\"><part-name>test</part-name>");
                    write(&mut fragment, &value);
                    fragment.push_str("</score-part></part-list></score-partwise>");
                    let mut again = None;
                    xml::walk(Cursor::new(fragment), |n, _, phase| {
                        if phase == xml::Phase::Complete && n.name == "part-list" {
                            again = read(n.child("score-part").unwrap(), &mut MappingReport::default())?;
                        }
                        Ok(())
                    })?;
                    assert_eq!(again, Some((identity, value)));
                    count += 1;
                }
                Ok(())
            }).unwrap();
            assert_eq!(
                scan.fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hashes[movement - 1]
            );
            assert_eq!(count, if movement == 4 { 23 } else { 18 });
            total += count;
        }
        assert_eq!(
            (total, names_with_spaces, sounds.len(), ports),
            (77, 27, 17, [2, 47, 24, 4])
        );
    }
}
