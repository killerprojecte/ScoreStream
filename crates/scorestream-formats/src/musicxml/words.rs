//! Plain authored score instructions; no inferred gain, tempo, timbre or navigation.
use super::{
    geometry::{decimal_text, exact_decimal},
    read::error,
    xml::{Node, escape},
};
use crate::MappingReport;
use scorestream_core::{
    Rational,
    notation::Placement,
    words::{WordsDirection, WordsText},
};
use std::{collections::BTreeMap, fmt::Write};
pub(super) fn read_text(w: &Node) -> Result<WordsText, MappingReport> {
    let number = |key| {
        w.attr(key)
            .map(|_| exact_decimal(&w.scalar_attribute(key)))
            .transpose()
    };
    Ok(WordsText {
        text: w.text.clone(),
        font_family: w.attr("font-family").map(str::to_owned),
        font_style: w.attr("font-style").map(str::to_owned),
        font_weight: w.attr("font-weight").map(str::to_owned),
        font_size: number("font-size")?,
        default_x: number("default-x")?,
        default_y: number("default-y")?,
        relative_x: number("relative-x")?,
        relative_y: number("relative-y")?,
    })
}
pub(super) fn write_text(w: &WordsText, out: &mut String) {
    out.push_str("<direction-type>");
    write_row(w, out);
    out.push_str("</direction-type>");
}
fn write_row(w: &WordsText, out: &mut String) {
    out.push_str("<words");
    for (key, value) in [
        ("font-family", w.font_family.as_deref()),
        ("font-style", w.font_style.as_deref()),
        ("font-weight", w.font_weight.as_deref()),
    ] {
        if let Some(value) = value {
            write!(out, " {key}=\"{}\"", escape(value)).unwrap();
        }
    }
    for (key, value) in [
        ("font-size", &w.font_size),
        ("default-x", &w.default_x),
        ("default-y", &w.default_y),
        ("relative-x", &w.relative_x),
        ("relative-y", &w.relative_y),
    ] {
        if let Some(value) = value {
            write!(
                out,
                " {key}=\"{}\"",
                decimal_text(value).expect("validated words decimal")
            )
            .unwrap();
        }
    }
    write!(out, ">{}</words>", escape(&w.text)).unwrap();
}
pub(super) fn read(
    n: &Node,
    part: &str,
    measure: &str,
    cursor: &Rational,
    divisions: Option<&Rational>,
) -> Result<Option<WordsDirection>, MappingReport> {
    if n.children
        .iter()
        .filter(|n| n.name == "direction-type")
        .count()
        != 1
    {
        return Ok(None);
    }
    let Some(w) = n.child("direction-type").and_then(|d| d.child("words")) else {
        return Ok(None);
    };
    if n.child("direction-type")
        .unwrap()
        .children
        .iter()
        .any(|row| row.name != "words")
    {
        return Ok(None);
    }
    let staff = n
        .child("staff")
        .map(|s| {
            s.text
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|s| *s > 0)
                .ok_or_else(|| error(n, "words.staff", "Expected positive staff"))
        })
        .transpose()?;
    let placement = match n.attr("placement") {
        None => None,
        Some("above") => Some(Placement::Above),
        Some("below") => Some(Placement::Below),
        _ => return Err(error(n, "words.placement", "Expected above or below")),
    };
    let (offset, _) = super::tempo::locations(n, &Rational::zero(), cursor, divisions)?;
    let pizzicato = if let Some(sound) = n.child("sound").filter(|s| s.attr("pizzicato").is_some())
    {
        if sound.attrs.len() != 1 {
            return Err(error(
                sound,
                "pizzicato.sound_attributes",
                "This sound owner profile contains only explicit pizzicato; tempo/dynamics are not merged or discarded",
            ));
        }
        Some(scorestream_core::words::PizzicatoSound {
            mode: scorestream_core::words::PizzicatoMode::parse(sound.attr("pizzicato").unwrap())
                .ok_or_else(|| error(sound, "pizzicato.value", "Expected exact yes or no"))?,
            at: super::tempo::locations(n, &Rational::zero(), cursor, divisions)?.1,
        })
    } else {
        None
    };
    Ok(Some(WordsDirection {
        pizzicato,
        continuations: n
            .child("direction-type")
            .unwrap()
            .children
            .iter()
            .skip(1)
            .map(read_text)
            .collect::<Result<_, _>>()?,
        part: part.into(),
        measure: measure.into(),
        offset,
        staff,
        placement,
        system: n.attr("system").map(str::to_owned),
        words: read_text(w)?,
    }))
}
pub(super) struct Export<'a> {
    by_measure: BTreeMap<(&'a str, &'a str), Vec<&'a WordsDirection>>,
}
impl<'a> Export<'a> {
    pub fn new(values: &'a [WordsDirection]) -> Self {
        let mut by_measure: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for e in values {
            by_measure
                .entry((e.part.as_str(), e.measure.as_str()))
                .or_default()
                .push(e);
        }
        Self { by_measure }
    }
    pub fn offsets(&self, part: &str, measure: &str) -> Vec<Rational> {
        self.by_measure
            .get(&(part, measure))
            .into_iter()
            .flatten()
            .flat_map(|e| {
                std::iter::once(e.offset.clone()).chain(e.pizzicato.iter().map(|p| p.at.clone()))
            })
            .collect()
    }
    pub fn write(&self, part: &str, measure: &str, div: u64, out: &mut String) {
        for e in self.by_measure.get(&(part, measure)).into_iter().flatten() {
            out.push_str("      <direction");
            if let Some(p) = e.placement {
                write!(
                    out,
                    " placement=\"{}\"",
                    match p {
                        Placement::Above => "above",
                        Placement::Below => "below",
                    }
                )
                .unwrap();
            }
            if let Some(s) = &e.system {
                write!(out, " system=\"{}\"", escape(s)).unwrap();
            }
            out.push('>');
            out.push_str("<direction-type>");
            write_row(&e.words, out);
            for row in &e.continuations {
                write_row(row, out);
            }
            out.push_str("</direction-type>");
            write!(
                out,
                "<offset sound=\"no\">{}</offset>",
                super::write::units(&e.offset, div)
            )
            .unwrap();
            if let Some(s) = e.staff {
                write!(out, "<staff>{s}</staff>").unwrap();
            }
            if let Some(p) = &e.pizzicato {
                write!(
                    out,
                    "<sound pizzicato=\"{}\"><offset>{}</offset></sound>",
                    p.mode.name(),
                    super::write::units(&p.at, div)
                )
                .unwrap();
            }
            out.push_str("</direction>\n");
        }
    }
}

#[cfg(test)]
mod corpus {
    use super::super::{MusicXml, xml};
    use super::*;
    use crate::{ScoreAdapter, sfs::SfsAdapter};
    use scorestream_core::{
        Pitch,
        notation::{EventId, PartId, VoiceId},
        score::*,
    };
    use std::{fs::File, io::BufReader, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to pinned original XML/MXL directory"]
    fn original_286_standalone_words_preserve_text_styles_and_positions() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let template=MusicXml.read(r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions></attributes><note id="e"><rest/><duration>4</duration><voice>1</voice></note></measure></part></score-partwise>"#).unwrap().value;
        for (i, hash) in hashes.iter().enumerate() {
            let mut parts = BTreeMap::<String, ScorePart>::new();
            let mut divisions = BTreeMap::<String, Rational>::new();
            let mut endpoints = vec![];
            let mut pizzicato_sound = 0;
            let mut mixed_words = 0;
            let scan = xml::walk(
                BufReader::new(
                    File::open(root.join(format!("Beethoven_Op.67_{}.musicxml", i + 1))).unwrap(),
                ),
                |m, parent, phase| {
                    if phase != xml::Phase::Complete || m.name != "measure" {
                        return Ok(());
                    }
                    let pid = parent.unwrap().attr("id").unwrap().to_owned();
                    let part = parts.entry(pid.clone()).or_insert_with(|| {
                        let mut p = template.parts[0].clone();
                        p.id = PartId(pid.clone());
                        p.voices[0].id = VoiceId(format!("v{pid}"));
                        p.instrument_metadata.as_mut().unwrap().source_id = format!("i{pid}");
                        p.measures.clear();
                        p
                    });
                    let ordinal = part.measures.len() + 1;
                    let mid = format!("m{pid}_{ordinal}");
                    let start = part
                        .measures
                        .last()
                        .map(|m| &m.start + &m.length)
                        .unwrap_or_default();
                    let mut cursor = Rational::zero();
                    let mut extent = Rational::zero();
                    let mut previous = Rational::zero();
                    for n in &m.children {
                        if n.name == "direction"
                            && n.child("direction-type")
                                .and_then(|d| d.child("words"))
                                .is_some()
                            && (n
                                .children
                                .iter()
                                .filter(|d| d.name == "direction-type")
                                .count()
                                > 1
                                || n.child("direction-type")
                                    .is_some_and(|d| d.children.len() != 1))
                            && !n.children.iter().any(|d| d.child("dashes").is_some())
                        {
                            mixed_words += 1;
                            let mut report = MappingReport::default();
                            xml::schema(n, &mut report);
                            let supported_group = n.children.iter().any(|d| d.child("metronome").is_some()) || n
                                .children
                                .iter()
                                .filter(|d| d.name == "direction-type")
                                .count()
                                == 1
                                && n.child("direction-type")
                                    .unwrap()
                                    .children
                                    .iter()
                                    .all(|w| w.name == "words")
                                && n.child("sound").is_none();
                            assert_eq!(report.has_errors(), !supported_group);
                        }

                        match n.name.as_str() {
                            "attributes" => {
                                if let Some(d) = n.value("divisions") {
                                    divisions.insert(pid.clone(), d.parse().unwrap());
                                }
                            }
                            "note" if n.child("grace").is_none() => {
                                let duration =
                                    &n.value("duration").unwrap().parse::<Rational>().unwrap()
                                        / &divisions[&pid];
                                if n.child("chord").is_none() {
                                    previous = cursor.clone();
                                    cursor = &cursor + &duration;
                                }
                                extent = extent.max(&previous + &duration).max(cursor.clone());
                            }
                            "backup" | "forward" => {
                                let duration =
                                    &n.value("duration").unwrap().parse::<Rational>().unwrap()
                                        / &divisions[&pid];
                                cursor = if n.name == "backup" {
                                    &cursor - &duration
                                } else {
                                    &cursor + &duration
                                };
                                extent = extent.max(cursor.clone());
                            }
                            "direction"
                                if n.children
                                    .iter()
                                    .filter(|d| d.name == "direction-type")
                                    .count()
                                    == 1
                                    && n.child("direction-type")
                                        .is_some_and(|d| d.children.len() == 1)
                                    && n.child("direction-type")
                                        .and_then(|d| d.child("words"))
                                        .is_some() =>
                            {
                                let mut report = MappingReport::default();
                                xml::schema(n, &mut report);
                                let pizz = n
                                    .child("sound")
                                    .is_some_and(|s| s.attr("pizzicato").is_some());
                                if pizz {
                                    pizzicato_sound += 1;
                                }
                                assert!(!report.has_errors(), "{report:?}");
                                endpoints.push(
                                    read(n, &pid, &mid, &cursor, divisions.get(&pid))?.unwrap(),
                                );
                            }
                            _ => {}
                        }
                    }
                    let mut measure = template.parts[0].measures[0].clone();
                    measure.id = mid;
                    measure.label = m.attr("number").unwrap().into();
                    measure.start = start;
                    measure.length = extent.clone();
                    measure.states.clear();
                    measure.events[0].id = EventId(format!("e{pid}_{ordinal}"));
                    measure.events[0].voice = part.voices[0].id.clone();
                    measure.events[0].duration = extent;
                    measure.events[0].pitch = Pitch::Rest;
                    part.measures.push(measure);
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(
                &scan
                    .fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hash
            );
            assert_eq!(endpoints.len(), [11, 163, 38, 74][i]);
            if endpoints.is_empty() {
                println!("movement {}: 0 endpoints; original hash {}", i + 1, hash);
                continue;
            }
            let wanted: std::collections::BTreeSet<_> =
                endpoints.iter().map(|e| e.part.clone()).collect();
            let mut model = ScoreDocument::default();
            model.parts = parts
                .into_values()
                .filter(|p| wanted.contains(&p.id.0))
                .collect();
            for (index, part) in model.parts.iter_mut().enumerate() {
                let voice = VoiceId(format!("v{}_1", index + 1));
                part.voices[0].id = voice.clone();
                for measure in &mut part.measures {
                    for event in &mut measure.events {
                        event.voice = voice.clone();
                    }
                }
            }
            let positions: BTreeMap<_, _> = model
                .parts
                .iter()
                .flat_map(|p| {
                    p.measures
                        .iter()
                        .map(move |m| ((p.id.0.as_str(), m.id.as_str()), &m.start))
                })
                .collect();
            endpoints.sort_by(|a, b| {
                a.part.cmp(&b.part).then(
                    positions[&(a.part.as_str(), a.measure.as_str())]
                        .cmp(positions[&(b.part.as_str(), b.measure.as_str())]),
                )
            });
            model.words = endpoints;
            assert_eq!(pizzicato_sound, [8, 0, 6, 10][i]);
            assert_eq!(
                model.words.iter().filter(|w| w.pizzicato.is_some()).count(),
                pizzicato_sound
            );
            assert_eq!(mixed_words, [1, 2, 1, 4][i]);
            // Named family projection retains real measure extents/endpoint offsets, not original notes or import claims.
            let sfs = SfsAdapter.write(&model).unwrap().value;
            let restored = SfsAdapter.read(&sfs).unwrap().value;
            assert_eq!(restored, model);
            let exported = MusicXml.write(&restored).unwrap().value;
            assert_eq!(MusicXml.read(&exported).unwrap().value, model);
            let plan = scorestream_app::playback::plan_source(&sfs).unwrap();
            assert!(
                plan.diagnostics
                    .iter()
                    .any(|d| d.code == "words.authored_only")
            );
            println!(
                "movement {}: {} exact standalone words; {} explicit pizzicato sound states retained; original hash {}",
                i + 1,
                model.words.len(),
                pizzicato_sound,
                hash
            );
        }
    }
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to pinned originals"]
    fn original_movement_two_p14_ordered_words_group() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let mut found = None;
        let mut ordinal = 0;
        let scan = xml::walk(
            BufReader::new(File::open(root.join("Beethoven_Op.67_2.musicxml")).unwrap()),
            |m, parent, phase| {
                if phase != xml::Phase::Complete
                    || m.name != "measure"
                    || parent.and_then(|p| p.attr("id")) != Some("P14")
                {
                    return Ok(());
                }
                ordinal += 1;
                if m.attr("number") == Some("32") {
                    assert_eq!(ordinal, 33);
                    let n = m.child("direction").unwrap();
                    assert!(n.child("sound").is_none());
                    let mut report = MappingReport::default();
                    xml::schema(n, &mut report);
                    assert!(!report.has_errors(), "{report:?}");
                    found = read(n, "p", "m", &Rational::zero(), None)?;
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
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a"
        );
        let w = found.unwrap();
        assert_eq!(w.words.text, "sempre ");
        assert_eq!(w.words.default_y.as_ref().unwrap().to_string(), "-40");
        assert_eq!(w.words.relative_x.as_ref().unwrap().to_string(), "-98/25");
        assert_eq!(w.words.relative_y.as_ref().unwrap().to_string(), "-854/25");
        assert_eq!(w.continuations.len(), 1);
        assert_eq!(
            w.continuations[0]
                .text
                .chars()
                .map(u32::from)
                .collect::<Vec<_>>(),
            vec![0xe522, 0xe522]
        );
        assert_eq!(
            w.continuations[0].font_family.as_deref(),
            Some("Leland Text")
        );
        assert_eq!(w.placement, Some(Placement::Below));
        let source = r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions></attributes><note id="e"><rest/><duration>4</duration><voice>1</voice></note></measure></part></score-partwise>"#;
        let mut model = MusicXml.read(source).unwrap().value;
        model.words.push(w);
        let sfs = SfsAdapter.write(&model).unwrap().value;
        assert_eq!(SfsAdapter.read(&sfs).unwrap().value, model);
        let out = MusicXml.write(&model).unwrap().value;
        assert_eq!(MusicXml.read(&out).unwrap().value, model);
        assert_eq!(out.matches("<direction-type>").count(), 1);
        assert_eq!(out.matches("<words").count(), 2);
    }
}
