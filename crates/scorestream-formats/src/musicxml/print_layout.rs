//! Closed measure-start print layouts, not an engraving authority.
use super::{
    geometry::{decimal_text, exact_decimal},
    read::error,
    xml::Node,
};
use crate::MappingReport;
use scorestream_core::print_layout::{Layout, PrintLayout};

pub(super) fn read(n: &Node, part: &str, measure: &str) -> Result<PrintLayout, MappingReport> {
    let fail = || {
        error(
            n,
            "print.shape",
            "Expected one closed system/staff layout or empty print at the measure start",
        )
    };
    let new_page = n
        .attr("new-page")
        .map(|v| match v {
            "yes" => Ok(true),
            "no" => Ok(false),
            _ => Err(fail()),
        })
        .transpose()?;
    let page_number = n.attr("page-number").map(str::to_string);
    let layout = match n.children.as_slice() {
        [] => Layout::None,
        [system] if system.name == "system-layout" => {
            let [margins, top] = system.children.as_slice() else {
                return Err(fail());
            };
            let [left, right] = margins.children.as_slice() else {
                return Err(fail());
            };
            if margins.name != "system-margins"
                || left.name != "left-margin"
                || right.name != "right-margin"
                || top.name != "top-system-distance"
            {
                return Err(fail());
            }
            Layout::System {
                left_margin: exact_decimal(left)?,
                right_margin: exact_decimal(right)?,
                top_system_distance: exact_decimal(top)?,
            }
        }
        [staff] if staff.name == "staff-layout" => {
            let [distance] = staff.children.as_slice() else {
                return Err(fail());
            };
            if distance.name != "staff-distance" {
                return Err(fail());
            }
            let number = staff
                .attr("number")
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|n| *n > 0)
                .ok_or_else(fail)?;
            Layout::Staff {
                number,
                distance: exact_decimal(distance)?,
            }
        }
        _ => return Err(fail()),
    };
    Ok(PrintLayout {
        part: part.into(),
        measure: measure.into(),
        new_page,
        page_number,
        layout,
    })
}
fn attr(s: &str) -> String {
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
pub(super) fn write(p: &PrintLayout) -> String {
    let mut out = String::from("      <print");
    if let Some(v) = p.new_page {
        out.push_str(if v {
            " new-page=\"yes\""
        } else {
            " new-page=\"no\""
        });
    }
    if let Some(v) = &p.page_number {
        out.push_str(&format!(" page-number=\"{}\"", attr(v)));
    }
    out.push('>');
    let dec = |r| decimal_text(r).expect("validated layout decimal");
    match &p.layout {
        Layout::None=>{},
        Layout::System {left_margin,right_margin,top_system_distance}=>out.push_str(&format!("<system-layout><system-margins><left-margin>{}</left-margin><right-margin>{}</right-margin></system-margins><top-system-distance>{}</top-system-distance></system-layout>",dec(left_margin),dec(right_margin),dec(top_system_distance))),
        Layout::Staff {number,distance}=>out.push_str(&format!("<staff-layout number=\"{number}\"><staff-distance>{}</staff-distance></staff-layout>",dec(distance))),
    }
    out.push_str("</print>\n");
    out
}

#[cfg(test)]
mod corpus {
    use super::*;
    use crate::musicxml::xml;
    use std::{fs::File, io::Read, path::PathBuf};
    #[test]
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to untouched original XML/MXL; print-field receipt, not whole-score import"]
    fn original_print_nodes_decode_and_export_without_field_loss() {
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let mut counts = [0; 4];
        let mut variants = [0; 2];
        for movement in 1..=4 {
            let bytes =
                std::fs::read(root.join(format!("Beethoven_Op.67_{movement}.musicxml"))).unwrap();
            let archive = crate::mxl::open(
                File::open(root.join(format!("Beethoven_Op.67_{movement}.mxl"))).unwrap(),
            )
            .unwrap();
            let mut paired = vec![];
            archive.into_reader().read_to_end(&mut paired).unwrap();
            assert_eq!(bytes, paired);
            let scan=xml::walk(bytes.as_slice(),|m,parent,phase|{
                if phase!=xml::Phase::Complete || m.name!="measure" {return Ok(());}
                let part=parent.unwrap().attr("id").unwrap();
                for n in m.children_named("print") {
                    let mut report=MappingReport::default();xml::schema(n,&mut report);assert!(!report.has_errors(),"{report:?}");
                    let measure=m.attr("number").unwrap();
                    let value=read(n,part,measure)?;
                    assert_eq!(value.new_page,n.attr("new-page").map(|s|s=="yes"));
                    assert_eq!(value.page_number.as_deref(),n.attr("page-number"));
                    match value.layout {Layout::System{..}=>variants[0]+=1,Layout::Staff{..}=>variants[1]+=1,_=>panic!("unexpected original variant")};
                    let exported=format!("<score-partwise><part id=\"{}\"><measure number=\"{}\">{}</measure></part></score-partwise>",attr(part),attr(measure),write(&value));
                    let mut recovered=None;
                    xml::walk(exported.as_bytes(),|m,parent,phase|{if phase==xml::Phase::Complete && m.name=="measure" {recovered=Some(read(m.child("print").unwrap(),parent.unwrap().attr("id").unwrap(),m.attr("number").unwrap())?);}Ok(())})?;
                    assert_eq!(Some(value),recovered);counts[movement-1]+=1;
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
        }
        assert_eq!(counts.iter().sum::<usize>(), 3355);
        assert_eq!(variants, [170, 3185]);
        eprintln!(
            "Pinned original print receipts per movement: {counts:?}; system/staff: {variants:?}. Not whole-score import."
        );
    }
}
