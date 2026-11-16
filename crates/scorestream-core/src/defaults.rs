//! Typed, preserve-only MusicXML score-wide engraving defaults.
use crate::{Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const EXTENSION: &str = "scorestream.defaults@1";
pub const KIND: &str = "scorestream.defaults";
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Defaults {
    pub scaling: Option<Scaling>,
    pub page_layout: Option<PageLayout>,
    pub appearance: Option<Vec<Appearance>>,
    pub music_font: Option<Font>,
    pub word_font: Option<Font>,
    pub lyric_fonts: Vec<Font>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scaling {
    pub millimeters: Rational,
    pub tenths: Rational,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageLayout {
    pub dimensions: Option<PageDimensions>,
    pub margins: Vec<PageMargins>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageDimensions {
    pub height: Rational,
    pub width: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageMargins {
    pub margin_type: Option<MarginType>,
    pub left: Rational,
    pub right: Rational,
    pub top: Rational,
    pub bottom: Rational,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Font {
    pub family: Option<String>,
    pub size: Option<Rational>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Appearance {
    LineWidth {
        line_type: LineWidthType,
        value: Rational,
    },
    NoteSize {
        size_type: NoteSizeType,
        value: Rational,
    },
}
macro_rules! named_enum {
    ($name:ident {$($variant:ident => $wire:literal),+ $(,)?}) => {
        #[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
        pub enum $name { $(#[serde(rename=$wire)] $variant),+ }
        impl $name {
            pub fn name(self)->&'static str { match self { $(Self::$variant=>$wire),+ } }
            pub fn from_name(s:&str)->Option<Self> { match s { $($wire=>Some(Self::$variant),)+ _=>None } }
        }
    }
}
named_enum!(MarginType { Odd=>"odd", Even=>"even", Both=>"both" });
named_enum!(NoteSizeType { Cue=>"cue", Grace=>"grace", GraceCue=>"grace-cue" });
named_enum!(LineWidthType {
    LightBarline=>"light barline", HeavyBarline=>"heavy barline", Beam=>"beam", Bracket=>"bracket",
    Dashes=>"dashes", Enclosure=>"enclosure", Ending=>"ending", Extend=>"extend", Leger=>"leger",
    Pedal=>"pedal", OctaveShift=>"octave shift", SlurMiddle=>"slur middle", SlurTip=>"slur tip",
    Staff=>"staff", Stem=>"stem", TieMiddle=>"tie middle", TieTip=>"tie tip", TupletBracket=>"tuplet bracket", Wedge=>"wedge"
});
fn xml_text(s: &str) -> bool {
    s.chars().all(|c|matches!(c,'\t'|'\n'|'\r'|'\u{20}'..='\u{d7ff}'|'\u{e000}'..='\u{fffd}'|'\u{10000}'..='\u{10ffff}'))
}
impl Font {
    pub fn valid(&self) -> bool {
        self.family.as_ref().is_none_or(|s| xml_text(s))
            && self
                .size
                .as_ref()
                .is_none_or(|v| v.is_positive() && crate::geometry::decimal(v))
    }
}
impl Defaults {
    pub fn valid(&self) -> bool {
        let decimal = crate::geometry::decimal;
        if self.scaling.as_ref().is_some_and(|s| {
            !s.millimeters.is_positive()
                || !s.tenths.is_positive()
                || !decimal(&s.millimeters)
                || !decimal(&s.tenths)
        }) {
            return false;
        }
        if let Some(p) = &self.page_layout {
            // MusicXML page-layout permits at most two margin groups; this is a shape rule, not a file-size cap.
            if p.margins.len() > 2
                || p.dimensions
                    .as_ref()
                    .is_some_and(|d| !decimal(&d.height) || !decimal(&d.width))
                || p.margins.iter().any(|m| {
                    [&m.left, &m.right, &m.top, &m.bottom]
                        .into_iter()
                        .any(|v| !decimal(v))
                })
            {
                return false;
            }
        }
        if let Some(appearance) = &self.appearance {
            let mut notes = false;
            for a in appearance {
                match a {
                    Appearance::LineWidth { value, .. } => {
                        if notes || !decimal(value) {
                            return false;
                        }
                    }
                    Appearance::NoteSize { value, .. } => {
                        notes = true;
                        if value.is_negative() || !decimal(value) {
                            return false;
                        }
                    }
                }
            }
        }
        self.music_font
            .iter()
            .chain(&self.word_font)
            .chain(&self.lyric_fonts)
            .all(Font::valid)
    }
}
pub fn is_kind(kind: &str) -> bool {
    kind == KIND || kind.starts_with("scorestream.defaults.")
}
fn put(a: &mut BTreeMap<String, Scalar>, key: &str, value: impl ToString) {
    a.insert(key.into(), Scalar::String(value.to_string()));
}
fn add(out: &mut Vec<Mark>, kind: &str, mut a: BTreeMap<String, Scalar>) {
    a.insert("index".into(), Scalar::Integer(out.len() as i64));
    put(&mut a, "kind", kind);
    out.push(Mark {
        target: "score".into(),
        kind: KIND.into(),
        attributes: a,
        source: SourceRange::default(),
    });
}
fn font_attributes(font: &Font) -> BTreeMap<String, Scalar> {
    let mut a = BTreeMap::new();
    if let Some(f) = &font.family {
        put(&mut a, "family", f);
    }
    if let Some(s) = &font.size {
        put(&mut a, "size", s);
    }
    a
}
pub fn encode(d: &Defaults) -> Vec<Mark> {
    let mut out = vec![];
    add(&mut out, "root", BTreeMap::new());
    if let Some(s) = &d.scaling {
        let mut a = BTreeMap::new();
        put(&mut a, "millimeters", &s.millimeters);
        put(&mut a, "tenths", &s.tenths);
        add(&mut out, "scaling", a);
    }
    if let Some(p) = &d.page_layout {
        let mut a = BTreeMap::new();
        if let Some(d) = &p.dimensions {
            put(&mut a, "height", &d.height);
            put(&mut a, "width", &d.width);
        }
        add(&mut out, "page_layout", a);
        for m in &p.margins {
            let mut a = BTreeMap::new();
            if let Some(t) = m.margin_type {
                put(&mut a, "margin_type", t.name());
            }
            for (k, v) in [
                ("left", &m.left),
                ("right", &m.right),
                ("top", &m.top),
                ("bottom", &m.bottom),
            ] {
                put(&mut a, k, v);
            }
            add(&mut out, "page_margins", a);
        }
    }
    if let Some(appearance) = &d.appearance {
        add(&mut out, "appearance", BTreeMap::new());
        for a in appearance {
            let mut fields = BTreeMap::new();
            let kind = match a {
                Appearance::LineWidth { line_type, value } => {
                    put(&mut fields, "line_type", line_type.name());
                    put(&mut fields, "value", value);
                    "line_width"
                }
                Appearance::NoteSize { size_type, value } => {
                    put(&mut fields, "size_type", size_type.name());
                    put(&mut fields, "value", value);
                    "note_size"
                }
            };
            add(&mut out, kind, fields);
        }
    }
    if let Some(f) = &d.music_font {
        add(&mut out, "music_font", font_attributes(f));
    }
    if let Some(f) = &d.word_font {
        add(&mut out, "word_font", font_attributes(f));
    }
    for f in &d.lyric_fonts {
        add(&mut out, "lyric_font", font_attributes(f));
    }
    out
}
pub fn decode(doc: &Document) -> Result<Option<Defaults>, Vec<Diagnostic>> {
    let fail = || {
        vec![Diagnostic::error(
            "defaults.wire",
            "Defaults require complete-score capability, time-zero score ownership, contiguous indices, closed typed fields and ordered nonduplicate sections",
            doc.source,
        )]
    };
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let mut marks = BTreeMap::new();
    for frame in &doc.frames {
        for record in &frame.records {
            let Record::Mark(m) = record else { continue };
            if !is_kind(&m.kind) {
                continue;
            }
            if !enabled
                || doc.document_kind != DocumentKind::Score
                || frame.time != Rational::zero()
                || m.target != "score"
                || m.kind != KIND
            {
                return Err(fail());
            }
            let index = m
                .attributes
                .get("index")
                .and_then(Scalar::as_i64)
                .filter(|i| *i >= 0)
                .ok_or_else(fail)?;
            if marks.insert(index, m).is_some() {
                return Err(fail());
            }
        }
    }
    if !enabled {
        return Ok(None);
    }
    if doc.document_kind != DocumentKind::Score
        || marks.is_empty()
        || marks.keys().copied().ne(0..marks.len() as i64)
    {
        return Err(fail());
    }
    let mut d = Defaults::default();
    let mut previous = 0;
    for (i, m) in marks.values().enumerate() {
        let a = &m.attributes;
        let text = |key: &str| -> Result<Option<String>, Vec<Diagnostic>> {
            a.get(key)
                .map(|v| v.as_str().map(str::to_owned).ok_or_else(fail))
                .transpose()
        };
        let number = |key: &str| -> Result<Option<Rational>, Vec<Diagnostic>> {
            text(key)?
                .map(|v| v.parse().map_err(|_| fail()))
                .transpose()
        };
        let required =
            |key: &str| -> Result<Rational, Vec<Diagnostic>> { number(key)?.ok_or_else(fail) };
        let kind = text("kind")?.ok_or_else(fail)?;
        if i == 0 {
            if kind != "root" {
                return Err(fail());
            }
            continue;
        }
        let rank = match kind.as_str() {
            "scaling" => 1,
            "page_layout" | "page_margins" => 2,
            "appearance" | "line_width" | "note_size" => 3,
            "music_font" => 4,
            "word_font" => 5,
            "lyric_font" => 6,
            _ => return Err(fail()),
        };
        if rank < previous {
            return Err(fail());
        }
        previous = rank;
        match kind.as_str() {
            "scaling" => {
                if d.scaling
                    .replace(Scaling {
                        millimeters: required("millimeters")?,
                        tenths: required("tenths")?,
                    })
                    .is_some()
                {
                    return Err(fail());
                }
            }
            "page_layout" => {
                let dimensions = match (number("height")?, number("width")?) {
                    (None, None) => None,
                    (Some(height), Some(width)) => Some(PageDimensions { height, width }),
                    _ => return Err(fail()),
                };
                if d.page_layout
                    .replace(PageLayout {
                        dimensions,
                        margins: vec![],
                    })
                    .is_some()
                {
                    return Err(fail());
                }
            }
            "page_margins" => {
                let m = PageMargins {
                    margin_type: text("margin_type")?
                        .map(|s| MarginType::from_name(&s).ok_or_else(fail))
                        .transpose()?,
                    left: required("left")?,
                    right: required("right")?,
                    top: required("top")?,
                    bottom: required("bottom")?,
                };
                d.page_layout.as_mut().ok_or_else(fail)?.margins.push(m);
            }
            "appearance" => {
                if d.appearance.replace(vec![]).is_some() {
                    return Err(fail());
                }
            }
            "line_width" => {
                let line_type = text("line_type")?
                    .and_then(|s| LineWidthType::from_name(&s))
                    .ok_or_else(fail)?;
                d.appearance
                    .as_mut()
                    .ok_or_else(fail)?
                    .push(Appearance::LineWidth {
                        line_type,
                        value: required("value")?,
                    });
            }
            "note_size" => {
                let size_type = text("size_type")?
                    .and_then(|s| NoteSizeType::from_name(&s))
                    .ok_or_else(fail)?;
                d.appearance
                    .as_mut()
                    .ok_or_else(fail)?
                    .push(Appearance::NoteSize {
                        size_type,
                        value: required("value")?,
                    });
            }
            "music_font" | "word_font" | "lyric_font" => {
                let font = Font {
                    family: text("family")?,
                    size: number("size")?,
                };
                match kind.as_str() {
                    "music_font" => {
                        if d.music_font.replace(font).is_some() {
                            return Err(fail());
                        }
                    }
                    "word_font" => {
                        if d.word_font.replace(font).is_some() {
                            return Err(fail());
                        }
                    }
                    _ => d.lyric_fonts.push(font),
                }
            }
            _ => unreachable!(),
        }
    }
    // Re-encoding checks closed fields and canonical parent/order. Rational fields
    // follow the shared native language's reduced exact spelling, not XML decimals.
    if !d.valid()
        || encode(&d)
            .iter()
            .map(|m| &m.attributes)
            .ne(marks.values().map(|m| &m.attributes))
    {
        return Err(fail());
    }
    Ok(Some(d))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn r(s: &str) -> Rational {
        s.parse().unwrap()
    }
    fn document(value: &Defaults) -> Document {
        let mut d = Document::new(DocumentKind::Score);
        d.required_extensions.push(EXTENSION.into());
        d.frames.push(crate::Frame {
            time: Rational::zero(),
            records: encode(value).into_iter().map(Record::Mark).collect(),
            source: SourceRange::default(),
        });
        d
    }
    fn root(d: &mut Document) -> &mut Mark {
        let Record::Mark(m) = &mut d.frames[0].records[0] else {
            unreachable!()
        };
        m
    }
    #[test]
    fn empty_absent_sections_and_ordered_repetitions_are_distinct() {
        let empty = Defaults::default();
        assert_eq!(decode(&document(&empty)).unwrap(), Some(empty.clone()));
        let absent = Document::new(DocumentKind::Score);
        assert_eq!(decode(&absent).unwrap(), None);
        let value = Defaults {
            page_layout: Some(PageLayout::default()),
            appearance: Some(vec![]),
            music_font: Some(Font::default()),
            lyric_fonts: vec![
                Font::default(),
                Font {
                    family: Some("\t FreeSerif\n&".into()),
                    size: None,
                },
            ],
            ..Default::default()
        };
        assert_ne!(value, empty);
        assert_eq!(decode(&document(&value)).unwrap(), Some(value));
    }
    #[test]
    fn exact_signed_tenths_and_zero_percent_survive_without_float_or_caps() {
        let value = Defaults {
            scaling: Some(Scaling {
                millimeters: r("13/250"),
                tenths: r("40"),
            }),
            page_layout: Some(PageLayout {
                dimensions: Some(PageDimensions {
                    height: r("0"),
                    width: r("-123456789012345678901234567890"),
                }),
                margins: vec![
                    PageMargins {
                        margin_type: None,
                        left: r("0"),
                        right: r("-1/8"),
                        top: r("1/2"),
                        bottom: r("20"),
                    },
                    PageMargins {
                        margin_type: Some(MarginType::Both),
                        left: r("0"),
                        right: r("0"),
                        top: r("0"),
                        bottom: r("0"),
                    },
                ],
            }),
            appearance: Some(vec![
                Appearance::LineWidth {
                    line_type: LineWidthType::SlurTip,
                    value: r("-1/2"),
                },
                Appearance::LineWidth {
                    line_type: LineWidthType::SlurTip,
                    value: r("0"),
                },
                Appearance::NoteSize {
                    size_type: NoteSizeType::GraceCue,
                    value: r("0"),
                },
            ]),
            word_font: Some(Font {
                family: Some("".into()),
                size: Some(r("100000000000000000000000000000000")),
            }),
            ..Default::default()
        };
        assert!(value.valid());
        let mut doc = document(&value);
        assert_eq!(decode(&doc).unwrap(), Some(value.clone()));
        let Record::Mark(m) = &mut doc.frames[0].records[1] else {
            unreachable!()
        };
        m.attributes
            .insert("tenths".into(), Scalar::String("80/2".into()));
        assert!(decode(&doc).is_err());
    }
    #[test]
    fn malformed_native_host_capability_and_closed_fields_reject() {
        let good = document(&Defaults::default());
        for mode in 0..7 {
            let mut d = good.clone();
            match mode {
                0 => d.required_extensions.clear(),
                1 => d.document_kind = DocumentKind::Fragment,
                2 => d.frames[0].time = r("1"),
                3 => root(&mut d).target = "p".into(),
                4 => {
                    root(&mut d)
                        .attributes
                        .insert("index".into(), Scalar::Integer(1));
                }
                5 => {
                    root(&mut d)
                        .attributes
                        .insert("unexpected".into(), Scalar::Boolean(true));
                }
                _ => {
                    let duplicate = d.frames[0].records[0].clone();
                    d.frames[0].records.push(duplicate);
                }
            };
            assert!(decode(&d).is_err(), "mode {mode}");
        }
        let mut no_root = good.clone();
        no_root.frames[0].records.clear();
        assert!(decode(&no_root).is_err());
    }
    #[test]
    fn orphan_sections_unknown_types_and_non_decimal_values_reject() {
        let d = Defaults {
            appearance: Some(vec![
                Appearance::LineWidth {
                    line_type: LineWidthType::Beam,
                    value: r("1"),
                },
                Appearance::NoteSize {
                    size_type: NoteSizeType::Cue,
                    value: r("70"),
                },
            ]),
            ..Default::default()
        };
        let good = document(&d);
        for mode in 0..4 {
            let mut doc = good.clone();
            match mode {
                0 => {
                    doc.frames[0].records.remove(1);
                    for (i, v) in doc.frames[0].records.iter_mut().enumerate() {
                        let Record::Mark(m) = v else { unreachable!() };
                        m.attributes
                            .insert("index".into(), Scalar::Integer(i as i64));
                    }
                }
                1 => {
                    let Record::Mark(m) = &mut doc.frames[0].records[2] else {
                        unreachable!()
                    };
                    m.attributes
                        .insert("line_type".into(), Scalar::String("unregistered".into()));
                }
                2 => {
                    let Record::Mark(m) = &mut doc.frames[0].records[2] else {
                        unreachable!()
                    };
                    m.attributes
                        .insert("value".into(), Scalar::String("1/3".into()));
                }
                _ => {
                    doc.frames[0].records.swap(2, 3);
                    for (i, v) in doc.frames[0].records.iter_mut().enumerate() {
                        let Record::Mark(m) = v else { unreachable!() };
                        m.attributes
                            .insert("index".into(), Scalar::Integer(i as i64));
                    }
                }
            };
            assert!(decode(&doc).is_err(), "mode {mode}");
        }
    }
    #[test]
    fn duplicate_sections_orphan_margins_and_invalid_typed_values_reject() {
        let value = Defaults {
            scaling: Some(Scaling {
                millimeters: r("1"),
                tenths: r("40"),
            }),
            page_layout: Some(PageLayout::default()),
            appearance: Some(vec![]),
            music_font: Some(Font::default()),
            word_font: Some(Font::default()),
            ..Default::default()
        };
        let good = document(&value);
        for index in 0..6 {
            let mut d = good.clone();
            let duplicate = d.frames[0].records[index].clone();
            d.frames[0].records.insert(index + 1, duplicate);
            for (i, v) in d.frames[0].records.iter_mut().enumerate() {
                let Record::Mark(m) = v else { unreachable!() };
                m.attributes
                    .insert("index".into(), Scalar::Integer(i as i64));
            }
            assert!(decode(&d).is_err(), "duplicate section {index}");
        }
        let margin = PageMargins {
            margin_type: Some(MarginType::Odd),
            left: r("0"),
            right: r("0"),
            top: r("0"),
            bottom: r("0"),
        };
        let mut too_many = value.clone();
        too_many.page_layout.as_mut().unwrap().margins = vec![margin.clone(); 3];
        assert!(!too_many.valid());
        let mut with_margin = value.clone();
        with_margin
            .page_layout
            .as_mut()
            .unwrap()
            .margins
            .push(margin);
        let mut doc = document(&with_margin);
        doc.frames[0].records.remove(2);
        for (i, v) in doc.frames[0].records.iter_mut().enumerate() {
            let Record::Mark(m) = v else { unreachable!() };
            m.attributes
                .insert("index".into(), Scalar::Integer(i as i64));
        }
        assert!(decode(&doc).is_err());
        for bad in [r("0"), r("-1"), r("1/3")] {
            let mut d = value.clone();
            d.scaling.as_mut().unwrap().millimeters = bad.clone();
            assert!(!d.valid());
            d = value.clone();
            d.music_font.as_mut().unwrap().size = Some(bad);
            assert!(!d.valid());
        }
    }
}
