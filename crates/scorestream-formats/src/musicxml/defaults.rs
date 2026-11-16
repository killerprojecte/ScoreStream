//! Closed typed score-wide defaults. No foreign subtree or renderer payload is retained.
use super::{
    geometry::{decimal_text, exact_decimal},
    read::{error, unsupported},
    xml::Node,
};
use crate::MappingReport;
use scorestream_core::defaults::*;
fn ordered(n: &Node, names: &[&str]) -> Result<(), MappingReport> {
    let mut previous = 0;
    for c in &n.children {
        let rank = names.iter().position(|s| *s == c.name).ok_or_else(|| {
            unsupported(
                c,
                "defaults.child",
                "Unsupported defaults content cannot be discarded",
            )
        })?;
        if rank < previous {
            return Err(unsupported(
                c,
                "defaults.order",
                "Authored defaults children must retain the supported MusicXML order",
            ));
        }
        previous = rank;
    }
    Ok(())
}
fn need<'a>(n: &'a Node, name: &str) -> Result<&'a Node, MappingReport> {
    n.child(name)
        .ok_or_else(|| error(n, "defaults.required", format!("Missing {name}")))
}
fn font(n: &Node) -> Result<Font, MappingReport> {
    Ok(Font {
        family: n.attr("font-family").map(str::to_owned),
        size: n
            .attr("font-size")
            .map(|_| exact_decimal(&n.scalar_attribute("font-size")))
            .transpose()?,
    })
}
pub(super) fn read(n: &Node) -> Result<Defaults, MappingReport> {
    ordered(
        n,
        &[
            "scaling",
            "page-layout",
            "appearance",
            "music-font",
            "word-font",
            "lyric-font",
        ],
    )?;
    let mut d = Defaults::default();
    if let Some(s) = n.child("scaling") {
        ordered(s, &["millimeters", "tenths"])?;
        d.scaling = Some(Scaling {
            millimeters: exact_decimal(need(s, "millimeters")?)?,
            tenths: exact_decimal(need(s, "tenths")?)?,
        });
    }
    if let Some(p) = n.child("page-layout") {
        ordered(p, &["page-height", "page-width", "page-margins"])?;
        let dimensions = match (p.child("page-height"), p.child("page-width")) {
            (None, None) => None,
            (Some(h), Some(w)) => Some(PageDimensions {
                height: exact_decimal(h)?,
                width: exact_decimal(w)?,
            }),
            _ => {
                return Err(error(
                    p,
                    "defaults.page_dimensions",
                    "Page height and width must be both present or both absent",
                ));
            }
        };
        let mut margins = vec![];
        for m in p.children_named("page-margins") {
            ordered(
                m,
                &["left-margin", "right-margin", "top-margin", "bottom-margin"],
            )?;
            margins.push(PageMargins {
                margin_type: m
                    .attr("type")
                    .map(|s| {
                        MarginType::from_name(s).ok_or_else(|| {
                            unsupported(
                                m,
                                "defaults.margin_type",
                                "Supported margin types are odd/even/both",
                            )
                        })
                    })
                    .transpose()?,
                left: exact_decimal(need(m, "left-margin")?)?,
                right: exact_decimal(need(m, "right-margin")?)?,
                top: exact_decimal(need(m, "top-margin")?)?,
                bottom: exact_decimal(need(m, "bottom-margin")?)?,
            });
        }
        d.page_layout = Some(PageLayout {
            dimensions,
            margins,
        });
    }
    if let Some(a) = n.child("appearance") {
        ordered(a, &["line-width", "note-size"])?;
        d.appearance = Some(
            a.children
                .iter()
                .map(|n| {
                    let value = exact_decimal(n)?;
                    match n.name.as_str() {
                        "line-width" => Ok(Appearance::LineWidth {
                            line_type: n
                                .attr("type")
                                .and_then(LineWidthType::from_name)
                                .ok_or_else(|| {
                                    unsupported(
                                        n,
                                        "defaults.line_type",
                                        "Unsupported line-width type",
                                    )
                                })?,
                            value,
                        }),
                        "note-size" => Ok(Appearance::NoteSize {
                            size_type: n
                                .attr("type")
                                .and_then(NoteSizeType::from_name)
                                .ok_or_else(|| {
                                    unsupported(
                                        n,
                                        "defaults.note_type",
                                        "Supported note-size types are cue/grace/grace-cue",
                                    )
                                })?,
                            value,
                        }),
                        _ => Err(unsupported(
                            n,
                            "defaults.appearance",
                            "Unsupported appearance child",
                        )),
                    }
                })
                .collect::<Result<_, _>>()?,
        );
    }
    d.music_font = n.child("music-font").map(font).transpose()?;
    d.word_font = n.child("word-font").map(font).transpose()?;
    d.lyric_fonts = n
        .children_named("lyric-font")
        .map(font)
        .collect::<Result<_, _>>()?;
    if !d.valid() {
        return Err(error(
            n,
            "defaults.fields",
            "Invalid supported defaults fields or hierarchy; nothing is silently normalized or removed",
        ));
    }
    Ok(d)
}
fn escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            '\r' => "&#13;".into(),
            '\n' => "&#10;".into(),
            '\t' => "&#9;".into(),
            _ => c.to_string(),
        })
        .collect()
}
fn decimal(v: &scorestream_core::Rational) -> String {
    decimal_text(v).expect("validated defaults decimal")
}
fn value(out: &mut String, name: &str, v: &scorestream_core::Rational) {
    out.push_str(&format!("<{name}>{}</{name}>", decimal(v)));
}
fn write_font(out: &mut String, name: &str, f: &Font) {
    out.push_str(&format!("<{name}"));
    if let Some(f) = &f.family {
        out.push_str(&format!(" font-family=\"{}\"", escape(f)));
    }
    if let Some(s) = &f.size {
        out.push_str(&format!(" font-size=\"{}\"", decimal(s)));
    }
    out.push_str("/>");
}
pub(super) fn write(d: &Defaults, out: &mut String) {
    out.push_str("  <defaults>");
    if let Some(s) = &d.scaling {
        out.push_str("<scaling>");
        value(out, "millimeters", &s.millimeters);
        value(out, "tenths", &s.tenths);
        out.push_str("</scaling>");
    }
    if let Some(p) = &d.page_layout {
        out.push_str("<page-layout>");
        if let Some(d) = &p.dimensions {
            value(out, "page-height", &d.height);
            value(out, "page-width", &d.width);
        }
        for m in &p.margins {
            out.push_str("<page-margins");
            if let Some(t) = m.margin_type {
                out.push_str(&format!(" type=\"{}\"", t.name()));
            }
            out.push('>');
            value(out, "left-margin", &m.left);
            value(out, "right-margin", &m.right);
            value(out, "top-margin", &m.top);
            value(out, "bottom-margin", &m.bottom);
            out.push_str("</page-margins>");
        }
        out.push_str("</page-layout>");
    }
    if let Some(a) = &d.appearance {
        out.push_str("<appearance>");
        for child in a {
            let (name, kind, v) = match child {
                Appearance::LineWidth { line_type, value } => {
                    ("line-width", line_type.name(), value)
                }
                Appearance::NoteSize { size_type, value } => ("note-size", size_type.name(), value),
            };
            out.push_str(&format!("<{name} type=\"{kind}\">{}</{name}>", decimal(v)));
        }
        out.push_str("</appearance>");
    }
    if let Some(f) = &d.music_font {
        write_font(out, "music-font", f);
    }
    if let Some(f) = &d.word_font {
        write_font(out, "word-font", f);
    }
    for f in &d.lyric_fonts {
        write_font(out, "lyric-font", f);
    }
    out.push_str("</defaults>\n");
}

#[cfg(test)]
mod corpus {
    use super::*;
    use crate::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
    use std::io::Read;
    fn signature(n: &Node) -> String {
        format!(
            "{}{:?}[{}]({})",
            n.name,
            n.attrs,
            n.text.trim(),
            n.children
                .iter()
                .map(signature)
                .collect::<Vec<_>>()
                .join("|")
        )
    }
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to the pinned original XML/MXL directory"]
    fn original_four_defaults_exact_tree_projection_and_passive_audio() {
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let root = std::path::PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let base = r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="piano"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><note id="e"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#;
        let template = MusicXml.read(base).unwrap().value;
        for movement in 1..=4 {
            let bytes =
                std::fs::read(root.join(format!("Beethoven_Op.67_{movement}.musicxml"))).unwrap();
            let archive = crate::mxl::open(
                std::fs::File::open(root.join(format!("Beethoven_Op.67_{movement}.mxl"))).unwrap(),
            )
            .unwrap();
            let mut paired = vec![];
            archive.into_reader().read_to_end(&mut paired).unwrap();
            assert_eq!(bytes, paired);
            let mut retained = None;
            let scan = super::super::xml::walk(bytes.as_slice(), |n, _, phase| {
                if phase == super::super::xml::Phase::Complete && n.name == "defaults" {
                    assert!(retained.is_none());
                    let mut r = MappingReport::default();
                    super::super::xml::schema(n, &mut r);
                    assert!(!r.has_errors());
                    retained = Some((read(n)?, signature(n)));
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(
                scan.fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hashes[movement - 1]
            );
            let (d, original) = retained.unwrap();
            assert_eq!(d.appearance.as_ref().unwrap().len(), 22);
            assert!(
                d.appearance.as_ref().unwrap()[..19]
                    .iter()
                    .all(|a| matches!(a, Appearance::LineWidth { .. }))
            );
            assert!(
                d.appearance.as_ref().unwrap()[19..]
                    .iter()
                    .all(|a| matches!(a, Appearance::NoteSize { .. }))
            );
            let margins = &d.page_layout.as_ref().unwrap().margins;
            assert_eq!(
                margins.iter().map(|m| m.margin_type).collect::<Vec<_>>(),
                vec![Some(MarginType::Even), Some(MarginType::Odd)]
            );
            assert_eq!(
                d.music_font.as_ref().unwrap().family.as_deref(),
                Some("Leland")
            );
            assert_eq!(
                d.word_font.as_ref().unwrap().family.as_deref(),
                Some("Edwin")
            );
            assert_eq!(d.lyric_fonts[0].family.as_deref(), Some("FreeSerif"));
            let mut score = template.clone();
            score.defaults = Some(d.clone());
            let native = SfsAdapter.write(&score).unwrap().value;
            let recovered = SfsAdapter.read(&native).unwrap().value;
            assert_eq!(score.defaults, recovered.defaults);
            let output = MusicXml.write(&recovered).unwrap().value;
            let again = MusicXml.read(&output).unwrap().value;
            assert_eq!(score.defaults, again.defaults);
            let mut emitted = None;
            super::super::xml::walk(output.as_bytes(), |n, _, phase| {
                if phase == super::super::xml::Phase::Complete && n.name == "defaults" {
                    emitted = Some(signature(n));
                }
                Ok(())
            })
            .unwrap();
            assert_eq!(
                Some(original),
                emitted,
                "Every source defaults child, attribute, numeric value and order must survive"
            );
            assert_eq!(
                scorestream_app::playback::plan_source(&native).unwrap(),
                scorestream_app::playback::plan_source(&SfsAdapter.write(&template).unwrap().value)
                    .unwrap()
            );
            eprintln!(
                "Original movement{movement}: exact defaults tree retained; scaling {},19 line widths,3 note sizes,2 ordered margin groups,3 fonts. Family projection only.",
                d.scaling.unwrap().millimeters
            );
        }
    }
}
