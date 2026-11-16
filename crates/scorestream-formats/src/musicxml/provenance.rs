//! Closed identification/encoding provenance. Supports rows remain descriptive claims.
use super::{
    read::error,
    xml::{Node, escape},
};
use crate::MappingReport;
use scorestream_core::provenance::{EncodingEntry, Provenance, token};
use std::fmt::Write;
pub(super) fn read(n: &Node) -> Result<Option<Provenance>, MappingReport> {
    for container in [n.child("encoding"), n.child("miscellaneous")]
        .into_iter()
        .flatten()
    {
        if !container
            .text
            .trim_matches([' ', '\t', '\r', '\n'])
            .is_empty()
        {
            return Err(error(
                container,
                "provenance.shape",
                "Container must not contain substantive text",
            ));
        }
    }
    let encoding = n
        .child("encoding")
        .map(|encoding| {
            encoding
                .children
                .iter()
                .map(|child| {
                    if child.name == "supports"
                        && !child.text.trim_matches([' ', '\t', '\r', '\n']).is_empty()
                    {
                        return Err(error(child, "provenance.shape", "Supports must be empty"));
                    }
                    Ok(match child.name.as_str() {
                        "software" => EncodingEntry::Software {
                            text: child.text.clone(),
                        },
                        "encoding-date" => EncodingEntry::EncodingDate {
                            date: child.text.clone(),
                        },
                        "supports" => EncodingEntry::Supports {
                            supported: match child.attr("type").map(token).as_deref() {
                                Some("yes") => true,
                                Some("no") => false,
                                _ => {
                                    return Err(error(
                                        child,
                                        "provenance.supports",
                                        "Supports type must be yes or no",
                                    ));
                                }
                            },
                            element: child
                                .attr("element")
                                .ok_or_else(|| {
                                    error(
                                        child,
                                        "provenance.supports",
                                        "Supports element is required",
                                    )
                                })?
                                .into(),
                            attribute: child.attr("attribute").map(String::from),
                            value: child.attr("value").map(String::from),
                        },
                        _ => {
                            return Err(error(
                                child,
                                "provenance.kind",
                                "Unsupported encoding child",
                            ));
                        }
                    })
                })
                .collect::<Result<Vec<_>, MappingReport>>()
        })
        .transpose()?;
    let miscellaneous_empty = n.child("miscellaneous").is_some();
    if encoding.is_none() && !miscellaneous_empty {
        return Ok(None);
    }
    let result = Provenance {
        encoding,
        miscellaneous_empty,
    };
    if !result.valid() {
        return Err(error(
            n,
            "provenance.value",
            "Invalid encoding date, XML text or supports declaration",
        ));
    }
    Ok(Some(result))
}
pub(super) fn write_encoding(p: &Option<Provenance>, out: &mut String) {
    let Some(entries) = p.as_ref().and_then(|p| p.encoding.as_ref()) else {
        return;
    };
    out.push_str("    <encoding>\n");
    for entry in entries {
        match entry {
            EncodingEntry::Software { text } => {
                writeln!(out, "      <software>{}</software>", escape(text)).unwrap()
            }
            EncodingEntry::EncodingDate { date } => {
                writeln!(out, "      <encoding-date>{}</encoding-date>", escape(date)).unwrap()
            }
            EncodingEntry::Supports {
                supported,
                element,
                attribute,
                value,
            } => {
                write!(
                    out,
                    "      <supports type=\"{}\" element=\"{}\"",
                    if *supported { "yes" } else { "no" },
                    escape(element)
                )
                .unwrap();
                if let Some(s) = attribute {
                    write!(out, " attribute=\"{}\"", escape(s)).unwrap();
                }
                if let Some(s) = value {
                    write!(out, " value=\"{}\"", escape(s)).unwrap();
                }
                out.push_str("/>\n");
            }
        }
    }
    out.push_str("    </encoding>\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ScoreAdapter,
        musicxml::{MusicXml, xml},
        sfs::SfsAdapter,
    };
    use std::{fs::File, io::BufReader, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to the four untouched pinned originals"]
    fn original_encoding_claims_and_empty_miscellaneous_roundtrip_in_authored_order() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        for movement in 1..=4 {
            let mut identification = None;
            let scan = xml::walk(
                BufReader::new(
                    File::open(root.join(format!("Beethoven_Op.67_{movement}.musicxml"))).unwrap(),
                ),
                |n, parent, phase| {
                    if phase == xml::Phase::Complete
                        && n.name == "identification"
                        && parent.is_some_and(|p| p.name == "score-partwise")
                    {
                        identification = Some(n.clone());
                    }
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                scan.fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hashes[movement - 1]
            );
            let n = identification.unwrap();
            let mut report = MappingReport::default();
            xml::schema(&n, &mut report);
            assert!(!report.has_errors(), "{report:?}");
            let p = read(&n).unwrap().unwrap();
            assert!(p.miscellaneous_empty);
            let mut expected = vec![
                EncodingEntry::Software {
                    text: "MuseScore Studio 4.7.3".into(),
                },
                EncodingEntry::EncodingDate {
                    date: if movement == 2 {
                        "2026-07-14"
                    } else {
                        "2026-07-11"
                    }
                    .into(),
                },
            ];
            for (element, attribute, value) in [
                ("accidental", None, None),
                ("beam", None, None),
                ("print", Some("new-page"), Some("yes")),
                ("print", Some("new-system"), Some("yes")),
                ("stem", None, None),
            ] {
                expected.push(EncodingEntry::Supports {
                    supported: true,
                    element: element.into(),
                    attribute: attribute.map(String::from),
                    value: value.map(String::from),
                });
            }
            assert_eq!(p.encoding.as_ref().unwrap(), &expected);
            let mut score=MusicXml.read(r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#).unwrap().value;
            score.provenance = Some(p.clone());
            let wire = SfsAdapter.write(&score).unwrap().value;
            let restored = SfsAdapter.read(&wire).unwrap().value;
            assert_eq!(restored.provenance, Some(p.clone()));
            let output = MusicXml.write(&restored).unwrap().value;
            assert_eq!(MusicXml.read(&output).unwrap().value.provenance, Some(p));
        }
    }
}
