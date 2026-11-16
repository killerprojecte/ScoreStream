//! Closed authored lyric sequence. Mixed text/elision/extend is not flattened.
use super::{
    geometry::{decimal_text, exact_decimal},
    read::error,
    xml::Node,
};
use crate::MappingReport;
use scorestream_core::lyrics::{EventLyrics, Lyric, Syllabic};
use std::collections::BTreeMap;
pub(super) fn read(note: &Node, event: &str) -> Result<Option<EventLyrics>, MappingReport> {
    let mut lyrics = vec![];
    for n in note.children_named("lyric") {
        let fail = || {
            error(
                n,
                "lyrics.shape",
                "Only one syllabic followed by one unformatted text node is supported; no lyric components are discarded",
            )
        };
        let [syllabic, text] = n.children.as_slice() else {
            return Err(fail());
        };
        if syllabic.name != "syllabic" || text.name != "text" {
            return Err(fail());
        }
        let syllabic = Syllabic::from_name(syllabic.text.trim_matches([' ', '\t', '\r', '\n']))
            .ok_or_else(fail)?;
        let number = n.attr("number").map(str::to_string);
        let color = n.attr("color").map(str::to_string);
        let decimal = |key| {
            n.attr(key)
                .map(|_| exact_decimal(&n.scalar_attribute(key)))
                .transpose()
        };
        let value = Lyric {
            number,
            syllabic,
            text: text.text.clone(),
            color,
            default_x: decimal("default-x")?,
            default_y: decimal("default-y")?,
            relative_x: decimal("relative-x")?,
            relative_y: decimal("relative-y")?,
        };
        if !value.valid() {
            return Err(fail());
        }
        lyrics.push(value);
    }
    Ok(if lyrics.is_empty() {
        None
    } else {
        Some(EventLyrics {
            event: event.into(),
            lyrics,
        })
    })
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
pub(super) fn write(l: &Lyric) -> String {
    let mut out = String::from("<lyric");
    for (name, v) in [("number", &l.number), ("color", &l.color)] {
        if let Some(v) = v {
            out.push_str(&format!(" {name}=\"{}\"", escape(v)));
        }
    }
    for (name, v) in [
        ("default-x", &l.default_x),
        ("default-y", &l.default_y),
        ("relative-x", &l.relative_x),
        ("relative-y", &l.relative_y),
    ] {
        if let Some(v) = v {
            out.push_str(&format!(
                " {name}=\"{}\"",
                decimal_text(v).expect("validated lyric decimal")
            ));
        }
    }
    out.push_str(&format!(
        "><syllabic>{}</syllabic><text>{}</text></lyric>",
        l.syllabic.name(),
        escape(&l.text)
    ));
    out
}
pub(super) struct Export {
    by_event: BTreeMap<String, String>,
}
impl Export {
    pub(super) fn new(groups: &[EventLyrics]) -> Self {
        Self {
            by_event: groups
                .iter()
                .map(|g| (g.event.clone(), g.lyrics.iter().map(write).collect()))
                .collect(),
        }
    }
    pub(super) fn event(&self, id: &str) -> &str {
        self.by_event.get(id).map(String::as_str).unwrap_or("")
    }
}

#[cfg(test)]
mod corpus {
    use super::*;
    use crate::musicxml::xml;
    use std::{fs::File, io::Read, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to untouched pinned XML/MXL; lyric field receipt, not full import"]
    fn original_lyric_nodes_roundtrip_with_owner_and_order() {
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let mut counts = [0; 4];
        let mut extras = [0; 2];
        for movement in 1..=4 {
            let bytes =
                std::fs::read(root.join(format!("Beethoven_Op.67_{movement}.musicxml"))).unwrap();
            let archive = crate::mxl::open(
                File::open(root.join(format!("Beethoven_Op.67_{movement}.mxl"))).unwrap(),
            )
            .unwrap();
            let mut paired = vec![];
            archive.into_reader().read_to_end(&mut paired).unwrap();
            assert_eq!(paired, bytes);
            let scan=xml::walk(bytes.as_slice(),|measure,_,phase|{
    if phase!=xml::Phase::Complete||measure.name!="measure"{return Ok(());}
    for note in measure.children_named("note") {
     let Some(group)=read(note,&note.path)? else {continue};
     for (raw,lyric) in note.children_named("lyric").zip(&group.lyrics) {
      let mut report=MappingReport::default();xml::schema(raw,&mut report);assert!(!report.has_errors(),"{report:?}");
      assert_eq!(lyric.text,raw.child("text").unwrap().text);assert_eq!(lyric.number.as_deref(),raw.attr("number"));assert_eq!(lyric.color.as_deref(),raw.attr("color"));
      for (key,value) in [("default-x",&lyric.default_x),("default-y",&lyric.default_y),("relative-x",&lyric.relative_x),("relative-y",&lyric.relative_y)] {assert_eq!(*value,raw.attr(key).map(|_|exact_decimal(&raw.scalar_attribute(key)).unwrap()));}
      extras[0]+=usize::from(raw.attr("default-x").is_some());extras[1]+=usize::from(raw.attr("relative-x").is_some());
     }
     let body:String=group.lyrics.iter().map(write).collect();
     let exported=format!("<score-partwise><part id=\"p\"><measure number=\"1\"><note>{body}</note></measure></part></score-partwise>");
     let mut recovered=None;
     xml::walk(exported.as_bytes(),|m,_,phase|{if phase==xml::Phase::Complete&&m.name=="measure"{recovered=read(m.child("note").unwrap(),&group.event)?;}Ok(())})?;
     assert_eq!(Some(&group),recovered.as_ref());counts[movement-1]+=group.lyrics.len();
    }Ok(())
   }).unwrap();
            assert_eq!(
                scan.fingerprint
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
                hashes[movement - 1]
            );
        }
        assert_eq!(counts, [225, 92, 114, 162]);
        assert_eq!(extras, [7, 2]);
        eprintln!(
            "Original lyric receipts: {counts:?}; rare default-x/relative-x: {extras:?}; not whole-score import."
        );
    }
}
