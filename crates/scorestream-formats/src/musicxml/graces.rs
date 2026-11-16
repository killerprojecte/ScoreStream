//! A bounded typed before-principal grace profile. No duration or silence is inferred.
use super::{
    read::{decimal, error, unsupported},
    type_name, write,
    xml::{Node, escape},
};
use crate::{Format, MappingEffect, MappingReport};
use num_traits::ToPrimitive;
use scorestream_core::{
    Pitch, Rational,
    notation::{EventId, VoiceId},
    score::*,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};
type Result<T> = std::result::Result<T, MappingReport>;
pub(super) struct Pending<'a> {
    pub source: &'a Node,
    pub note: ScoreGraceNote,
    voice: VoiceId,
    staff: u32,
}
pub(super) fn read<'a>(
    n: &'a Node,
    id: EventId,
    pitch: Pitch,
    written: Option<WrittenDuration>,
    voice: VoiceId,
    staff: u32,
    divisions: Option<&Rational>,
) -> Result<Pending<'a>> {
    let g = n
        .child("grace")
        .ok_or_else(|| error(n, "grace.missing", "Missing grace element"))?;
    if n.child("duration").is_some() {
        return Err(error(
            n,
            "grace.duration_choice",
            "Grace must not have measured duration",
        ));
    }
    if !matches!(pitch, Pitch::Pitched { .. })
        || n.child("chord").is_some()
        || n.child("cue").is_some()
        || n.child("tie").is_some()
        || n.children_named("notations")
            .any(|x| x.children.iter().any(|child| child.name != "slur"))
    {
        return Err(unsupported(
            n,
            "grace.profile",
            "Only pitched, non-chord grace notes without ties or notations other than bounded slurs are currently mapped",
        ));
    }
    let (slash, timing) = metadata(g, divisions)?;
    Ok(Pending {
        source: n,
        note: ScoreGraceNote {
            visible: super::appearance::visible(n)?,
            attack_dynamics: super::note_dynamics::read(n)?,
            id,
            anchor: EventId(String::new()),
            order: 0,
            pitch,
            written,
            slash,
            timing,
            stem: super::appearance::stem(n)?,
            accidental: super::accidentals::read(n)?,
        },
        voice,
        staff,
    })
}
/// Decode the actual grace element independently of unimplemented surrounding notation.
fn metadata(g: &Node, divisions: Option<&Rational>) -> Result<(Option<bool>, GraceTiming)> {
    let number = |key| -> Result<Option<Rational>> {
        g.attr(key)
            .map(|s| {
                let mut scalar = g.clone();
                scalar.text = s.into();
                decimal(&scalar)
            })
            .transpose()
    };
    let slash = match g.attr("slash") {
        None => None,
        Some("yes") => Some(true),
        Some("no") => Some(false),
        _ => return Err(error(g, "grace.slash", "Slash must be yes or no")),
    };
    let make_time = number("make-time")?;
    let timing = GraceTiming {
        steal_previous: number("steal-time-previous")?,
        steal_following: number("steal-time-following")?,
        make_time_quarters: match make_time {
            Some(v) => Some(
                &v / divisions.filter(|d| d.is_positive()).ok_or_else(|| {
                    error(
                        g,
                        "grace.divisions",
                        "Grace make-time requires explicit positive divisions",
                    )
                })?,
            ),
            None => None,
        },
    };
    if !timing.valid() {
        return Err(error(
            g,
            "grace.timing",
            "Steal percentages must be 0 through 100; this profile requires nonnegative make-time",
        ));
    }
    Ok((slash, timing))
}
pub(super) fn attach(
    pending: &mut Vec<Pending<'_>>,
    anchor: &ScoreEvent,
    out: &mut Vec<ScoreGraceNote>,
) -> Result<()> {
    let count = pending.len() as i64;
    for (i, mut g) in pending.drain(..).enumerate() {
        if g.voice != anchor.voice
            || g.staff != anchor.staff
            || !matches!(anchor.pitch, Pitch::Pitched { .. })
            || anchor.chord.is_some()
        {
            return Err(unsupported(
                g.source,
                "grace.anchor",
                "Grace group requires the immediately following pitched non-chord principal in the same voice and staff",
            ));
        }
        g.note.anchor = anchor.id.clone();
        g.note.order = i as i64 - count;
        out.push(g.note);
    }
    Ok(())
}
pub(super) struct Export<'a> {
    by_anchor: BTreeMap<&'a EventId, Vec<&'a ScoreGraceNote>>,
}
impl<'a> Export<'a> {
    pub(super) fn new(
        score: &'a ScoreDocument,
        ids: &mut BTreeSet<String>,
        r: &mut MappingReport,
    ) -> Self {
        let mut by_anchor = BTreeMap::<_, Vec<_>>::new();
        for g in &score.graces {
            if !write::xml_id(&g.id.0) || !ids.insert(g.id.0.clone()) {
                r.push(
                    Format::MusicXml,
                    MappingEffect::Unsupported,
                    "grace.identity",
                    &g.id.0,
                    "Grace needs a distinct XML-compatible ID",
                );
            }
            if matches!(&g.pitch, Pitch::Pitched { octave, alteration, .. }
                if !octave.to_i64().is_some_and(|o| (0..=9).contains(&o))
                    || write::decimal(alteration).is_none_or(|s| s.len() > 64))
            {
                r.push(
                    Format::MusicXml,
                    MappingEffect::Unsupported,
                    "grace.pitch",
                    &g.id.0,
                    "Grace pitch requires an XML octave and exact finite-decimal alteration within 64 characters",
                );
            }
            if [&g.timing.steal_previous, &g.timing.steal_following]
                .into_iter()
                .flatten()
                .any(|v| write::decimal(v).is_none_or(|s| s.len() > 64))
            {
                r.push(
                    Format::MusicXml,
                    MappingEffect::Unsupported,
                    "grace.timing_decimal",
                    &g.id.0,
                    "Grace percentages require exact bounded decimal output; no rounding",
                );
            }
            by_anchor.entry(&g.anchor).or_default().push(g);
        }
        for group in by_anchor.values_mut() {
            group.sort_by_key(|g| g.order);
        }
        crate::graces::authored_only(score, Format::MusicXml, r);
        Self { by_anchor }
    }
    pub(super) fn make_times(&self, m: &ScoreMeasure) -> Vec<&Rational> {
        m.events
            .iter()
            .flat_map(|e| {
                self.by_anchor
                    .get(&e.id)
                    .into_iter()
                    .flatten()
                    .filter_map(|g| g.timing.make_time_quarters.as_ref())
            })
            .collect()
    }
    /// The allocator and writer must see exactly the same grace encounter order.
    pub(super) fn before(&self, anchor: &EventId) -> impl Iterator<Item = &ScoreGraceNote> {
        self.by_anchor.get(anchor).into_iter().flatten().copied()
    }
    pub(super) fn validate_make_times(&self, m: &ScoreMeasure, div: u64, r: &mut MappingReport) {
        for event in &m.events {
            for g in self.by_anchor.get(&event.id).into_iter().flatten() {
                if g.timing.make_time_quarters.as_ref().is_some_and(|v| {
                    (v * &Rational::from_big_integer(div.into()))
                        .numerator()
                        .to_string()
                        .len()
                        > 64
                }) {
                    r.push(Format::MusicXml, MappingEffect::Unsupported,
                        "grace.make_time_precision", &g.id.0,
                        "Scaled grace make-time must fit the exact 64-character XML numeric profile; no rounding");
                }
            }
        }
    }
    pub(super) fn write_before(
        &self,
        anchor: &ScoreEvent,
        voice: &str,
        div: u64,
        beams: &super::beams::Export<'_>,
        slurs: &super::slurs::Export<'_>,
        geometry: &super::geometry::Export,
        lyrics: &super::lyrics::Export,
        out: &mut String,
    ) {
        for g in self.by_anchor.get(&anchor.id).into_iter().flatten() {
            write!(
                out,
                "      <note id=\"{}\"{}{}{}><grace",
                escape(&g.id.0),
                geometry.event(&g.id.0),
                super::note_dynamics::attribute(&g.attack_dynamics),
                super::appearance::visibility_attribute(g.visible)
            )
            .unwrap();
            if let Some(s) = g.slash {
                write!(out, " slash=\"{}\"", if s { "yes" } else { "no" }).unwrap();
            }
            for (key, v) in [
                ("steal-time-previous", &g.timing.steal_previous),
                ("steal-time-following", &g.timing.steal_following),
            ] {
                if let Some(v) = v {
                    write!(out, " {key}=\"{}\"", write::decimal(v).unwrap()).unwrap();
                }
            }
            if let Some(v) = &g.timing.make_time_quarters {
                write!(
                    out,
                    " make-time=\"{}\"",
                    (v * &Rational::from_big_integer(div.into())).numerator()
                )
                .unwrap();
            }
            out.push_str("/>");
            if let Pitch::Pitched {
                step,
                alteration,
                octave,
            } = &g.pitch
            {
                write!(
                    out,
                    "<pitch><step>{step}</step><alter>{}</alter><octave>{octave}</octave></pitch>",
                    write::decimal(alteration).unwrap()
                )
                .unwrap();
            }
            write!(out, "<voice>{}</voice>", escape(voice)).unwrap();
            if let Some(w) = &g.written {
                out.push_str("<type");
                if let Some(size) = w.size {
                    write!(out, " size=\"{}\"", size.name()).unwrap();
                }
                write!(out, ">{}</type>", type_name(w.note_type)).unwrap();
                for index in 0..w.dots {
                    write!(out, "<dot{}/>", geometry.dot(&g.id.0, index)).unwrap();
                }
                super::accidentals::write(g.accidental.as_ref(), out);
                if let Some(t) = &w.time_modification {
                    write!(out,"<time-modification><actual-notes>{}</actual-notes><normal-notes>{}</normal-notes>",t.actual,t.normal).unwrap();
                    if let Some(n) = t.normal_type {
                        write!(out, "<normal-type>{}</normal-type>", type_name(n)).unwrap();
                    }
                    for _ in 0..t.normal_dots {
                        out.push_str("<normal-dot/>");
                    }
                    out.push_str("</time-modification>");
                }
            }
            if g.written.is_none() {
                super::accidentals::write(g.accidental.as_ref(), out);
            }
            if let Some(stem) = g.stem {
                write!(out, "<stem>{}</stem>", stem.name()).unwrap();
            }
            write!(out, "<staff>{}</staff>", anchor.staff).unwrap();
            beams.write(&g.id, out);
            if slurs.contains(&g.id) {
                out.push_str("<notations>");
                slurs.write(&g.id, out);
                out.push_str("</notations>");
            }
            out.push_str(lyrics.event(&g.id.0));
            out.push_str("</note>\n");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::xml;
    use super::*;
    use std::{fs::File, io::BufReader, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to the pinned original XML/MXL directory"]
    fn original_beethoven_grace_inventory_keeps_authored_metadata_and_anchor_order() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        for movement in 1..=4 {
            let path = root.join(format!("Beethoven_Op.67_{movement}.musicxml"));
            let mut count = 0;
            let mut slash_yes = 0;
            let mut absent = 0;
            let mut groups = 0;
            let mut two = 0;
            let mut division = BTreeMap::<String, Rational>::new();
            let scan = xml::walk(
                BufReader::new(File::open(&path).unwrap()),
                |m, parent, phase| {
                    if phase != xml::Phase::Complete || m.name != "measure" {
                        return Ok(());
                    }
                    let part = parent.unwrap().attr("id").unwrap();
                    if let Some(d) = m.child("attributes").and_then(|a| a.child("divisions")) {
                        division.insert(part.into(), decimal(d)?);
                    }
                    for (i, n) in m.children.iter().enumerate() {
                        let Some(g) = n.child("grace") else { continue };
                        assert_eq!(n.name, "note");
                        assert!(n.child("duration").is_none());
                        assert!(n.child("pitch").is_some());
                        assert!(n.child("rest").is_none());
                        assert!(n.child("cue").is_none());
                        assert!(n.child("chord").is_none());
                        let mut schema = MappingReport::default();
                        xml::schema(g, &mut schema);
                        assert!(!schema.has_errors(), "{schema:?}");
                        let (slash, timing) = metadata(g, division.get(part))?;
                        assert_eq!(timing, GraceTiming::default());
                        count += 1;
                        slash_yes += usize::from(slash == Some(true));
                        absent += usize::from(slash.is_none());
                        let mut next = i + 1;
                        while next < m.children.len() && m.children[next].child("grace").is_some() {
                            next += 1;
                        }
                        let anchor = &m.children[next];
                        assert_eq!(anchor.name, "note");
                        assert!(anchor.child("duration").is_some());
                        assert!(anchor.child("pitch").is_some());
                        assert!(anchor.child("chord").is_none());
                        assert_eq!(n.value("voice"), anchor.value("voice"));
                        assert_eq!(n.value("staff"), anchor.value("staff"));
                        if i == 0 || m.children[i - 1].child("grace").is_none() {
                            groups += 1;
                            assert!((1..=2).contains(&(next - i)));
                            two += usize::from(next - i == 2);
                        }
                        // All surrounding authored notation remains intact. This metadata-only
                        // test does not strip stems, beams, slurs, accidentals or positions.
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
            assert_eq!(
                (count, slash_yes, absent, groups, two),
                [
                    (0, 0, 0, 0, 0),
                    (13, 7, 6, 10, 3),
                    (128, 4, 124, 66, 62),
                    (96, 32, 64, 64, 32)
                ][movement - 1]
            );
            assert_eq!(scan.notes, [15261, 10908, 11488, 30902][movement - 1]);
            assert!(super::super::read_reader(BufReader::new(File::open(path).unwrap())).is_err());
        }
    }
    #[test]
    fn grace_metadata_without_make_time_does_not_require_divisions() {
        let input = r#"<score-partwise version="4.0"><part-list/><part id="p"><measure number="1"><note><grace slash="no" steal-time-following="12.5"/></note></measure></part></score-partwise>"#;
        let mut seen = false;
        xml::walk(std::io::Cursor::new(input), |n, _, _| {
            if n.name == "measure" {
                let g = n.child("note").unwrap().child("grace").unwrap();
                let (slash, timing) = metadata(g, None)?;
                assert_eq!(slash, Some(false));
                assert_eq!(timing.steal_following, Some("25/2".parse().unwrap()));
                seen = true;
            }
            Ok(())
        })
        .unwrap();
        assert!(seen);
    }
}
