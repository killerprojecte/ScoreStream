use super::{
    geometry::{decimal_text, exact_decimal},
    xml::{Node, escape},
};
use crate::MappingReport;
use scorestream_core::credits::{Credit, Justify, VerticalAlign};
pub(super) fn read(n: &Node) -> Result<Credit, MappingReport> {
    let bad = || {
        super::read::error(
            n,
            "credits.shape",
            "Supported credit is optional credit-type followed by exactly one credit-words; page must be positive and alignment/font size valid",
        )
    };
    let names: Vec<_> = n.children.iter().map(|c| c.name.as_str()).collect();
    if names != ["credit-words"] && names != ["credit-type", "credit-words"] {
        return Err(bad());
    }
    let w = n.child("credit-words").ok_or_else(bad)?;
    let number = |key| {
        w.attr(key)
            .map(|_| exact_decimal(&w.scalar_attribute(key)))
            .transpose()
    };
    let c = Credit {
        page: n
            .attr("page")
            .map(|p| {
                p.trim_matches([' ', '\t', '\r', '\n'])
                    .parse()
                    .map_err(|_| bad())
            })
            .transpose()?,
        credit_type: n.child("credit-type").map(|t| t.text.clone()),
        text: w.text.clone(),
        default_x: number("default-x")?,
        default_y: number("default-y")?,
        font_size: number("font-size")?,
        justify: w
            .attr("justify")
            .map(|v| Justify::from_name(v).ok_or_else(bad))
            .transpose()?,
        valign: w
            .attr("valign")
            .map(|v| VerticalAlign::from_name(v).ok_or_else(bad))
            .transpose()?,
    };
    if !c.valid() {
        return Err(bad());
    }
    Ok(c)
}
pub(super) fn write(credits: &[Credit], out: &mut String) {
    for c in credits {
        out.push_str("  <credit");
        if let Some(page) = c.page {
            out.push_str(&format!(" page=\"{page}\""));
        }
        out.push_str(">\n");
        if let Some(t) = &c.credit_type {
            out.push_str(&format!("    <credit-type>{}</credit-type>\n", escape(t)));
        }
        out.push_str("    <credit-words");
        for (key, value) in [
            ("default-x", &c.default_x),
            ("default-y", &c.default_y),
            ("font-size", &c.font_size),
        ] {
            if let Some(v) = value {
                out.push_str(&format!(
                    " {key}=\"{}\"",
                    decimal_text(v).expect("validated decimal")
                ));
            }
        }
        for (key, value) in [
            ("justify", c.justify.map(|v| v.name())),
            ("valign", c.valign.map(|v| v.name())),
        ] {
            if let Some(v) = value {
                out.push_str(&format!(" {key}=\"{}\"", v));
            }
        }
        out.push_str(&format!(
            ">{}</credit-words>\n  </credit>\n",
            escape(&c.text)
        ));
    }
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
    #[ignore = "set SCORESTREAM_MXL_FIXTURES to pinned original XML/MXL directory"]
    fn original_beethoven_182_credits_are_typed_without_changing_originals() {
        let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
        let hashes = [
            "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
            "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
            "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
            "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
        ];
        for (i, count) in [42, 33, 45, 62].into_iter().enumerate() {
            let path = root.join(format!("Beethoven_Op.67_{}.musicxml", i + 1));
            let mut credits = vec![];
            let scan = xml::walk(
                BufReader::new(File::open(&path).unwrap()),
                |n, parent, phase| {
                    if phase == xml::Phase::Complete
                        && n.name == "credit"
                        && parent.is_some_and(|p| p.name == "score-partwise")
                    {
                        let mut report = MappingReport::default();
                        xml::schema(n, &mut report);
                        assert!(!report.has_errors(), "{report:?}");
                        let credit = read(n)?;
                        assert_eq!(credit.text, n.child("credit-words").unwrap().text);
                        credits.push(credit);
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
                hashes[i]
            );
            assert_eq!(credits.len(), count);
            assert_eq!(
                credits
                    .iter()
                    .filter(|c| c.credit_type.as_deref() == Some("rights"))
                    .count(),
                count - 3
            );
            assert_eq!(credits.iter().filter(|c| c.font_size.is_none()).count(), 1);
            // Only the credit family is projected onto a named minimal score.
            // The untouched complete original still has other unsupported families.
            let fixture = r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes><note><rest/><duration>4</duration><voice>1</voice><type>whole</type></note></measure></part></score-partwise>"#;
            let mut score = MusicXml.read(fixture).unwrap().value;
            score.credits = credits;
            let sfs = SfsAdapter.write(&score).unwrap().value;
            let recovered = SfsAdapter.read(&sfs).unwrap().value;
            assert_eq!(recovered, score);
            let exported = MusicXml.write(&recovered).unwrap().value;
            assert_eq!(MusicXml.read(&exported).unwrap().value, score);
            println!(
                "movement {}: {} credit nodes, exact source hash {}, typed/SFS/XML preserved",
                i + 1,
                count,
                hashes[i]
            );
        }
    }
}
