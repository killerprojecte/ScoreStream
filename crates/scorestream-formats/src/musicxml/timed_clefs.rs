//! Source-backed mid-bar clefs. Ordinary timed staff-state SFS, never note pitch changes.
use super::{
    read::{error, unsupported},
    write::units,
    xml::Node,
};
use crate::MappingReport;
use scorestream_core::{
    Rational,
    score::{Clef, ClefSign, ScoreMeasure, TimedClef},
};
use std::collections::BTreeSet;
pub(super) fn read(
    measure: &Node,
    index: usize,
    staff_count: u32,
    cursor: &Rational,
    extent: &Rational,
    divisions: Option<&Rational>,
    saw_backup: bool,
) -> Result<TimedClef, MappingReport> {
    let n = &measure.children[index];
    let voices: BTreeSet<_> = measure
        .children
        .iter()
        .filter(|n| n.name == "note" || n.child("voice").is_some())
        .map(|n| n.value("voice").unwrap_or("1"))
        .collect();
    if staff_count != 1
        || voices.len() > 1
        || n.children.len() != 1
        || n.children[0].name != "clef"
        || !cursor.is_positive()
    {
        return Err(unsupported(
            n,
            "attributes.mid_measure",
            "Only positive-offset clef-only attributes on a single-staff, single-voice measure are supported; other timed state is not relocated",
        ));
    }
    if saw_backup {
        let balanced = if let (Some(b), Some(f), Some(div)) = (
            index.checked_sub(1).map(|i| &measure.children[i]),
            measure.children.get(index + 1),
            divisions,
        ) {
            if b.name == "backup"
                && f.name == "forward"
                && b.attrs.is_empty()
                && f.attrs.is_empty()
                && b.children.len() == 1
                && f.children.len() == 1
                && b.children[0].name == "duration"
                && f.children[0].name == "duration"
            {
                let a = b.value("duration").and_then(|s| s.parse::<Rational>().ok());
                let z = f.value("duration").and_then(|s| s.parse::<Rational>().ok());
                a.as_ref().is_some_and(|a| {
                    a.is_positive() && z.as_ref() == Some(a) && cursor + &(a / div) == *extent
                })
            } else {
                false
            }
        } else {
            false
        };
        if !balanced {
            return Err(unsupported(
                n,
                "clef.cursor_ambiguous",
                "After backup only an immediate equal-duration backup/clef/forward excursion from the furthest cursor is supported; no whole-staff time is guessed",
            ));
        }
    }
    let c = &n.children[0];
    if c.children.len() != 2 || c.children[0].name != "sign" || c.children[1].name != "line" {
        return Err(unsupported(
            c,
            "clef.shape",
            "Only ordered sign and line are supported",
        ));
    }
    let staff = c
        .attr("number")
        .unwrap_or("1")
        .parse::<u32>()
        .map_err(|_| error(c, "clef.staff", "Invalid clef staff"))?;
    let sign = match c.value("sign") {
        Some("G") => ClefSign::G,
        Some("F") => ClefSign::F,
        Some("C") => ClefSign::C,
        _ => {
            return Err(unsupported(
                c,
                "clef.sign",
                "Only classic G/F/C clefs are mapped",
            ));
        }
    };
    let line = c
        .value("line")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| error(c, "clef.line", "Invalid clef line"))?;
    let result = TimedClef {
        offset: cursor.clone(),
        staff,
        clef: Clef { sign, line },
    };
    if staff != 1 || !result.valid_clef() {
        return Err(unsupported(
            c,
            "clef.fields",
            "Unsupported staff or classic clef line",
        ));
    }
    Ok(result)
}
pub(super) fn write(m: &ScoreMeasure, div: u64, out: &mut String) {
    // MusicXML defines mid-measure attributes in score order, not XML order.
    // The final cursor is measure.length; each exact closed excursion restores it.
    for c in &m.clef_changes {
        let delta = units(&(&m.length - &c.offset), div);
        let sign = match c.clef.sign {
            ClefSign::G => 'G',
            ClefSign::F => 'F',
            ClefSign::C => 'C',
        };
        out.push_str(&format!("      <backup><duration>{delta}</duration></backup>\n      <attributes><clef number=\"{}\"><sign>{sign}</sign><line>{}</line></clef></attributes>\n      <forward><duration>{delta}</duration></forward>\n",c.staff,c.clef.line));
    }
}
