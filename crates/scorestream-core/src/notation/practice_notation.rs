//! Practice-only admission of validated engraved grouping and tuplet topology.
//! Measured events remain the sole clock. No ornament realization is admitted here.
use crate::{Diagnostic, Document, Rational, Scalar};
pub(super) fn is_passive(extension: &str) -> bool {
    extension == crate::part_groups::EXTENSION || extension == crate::tuplets::EXTENSION
}
pub(super) fn validate(doc: &Document, maximum_bits: Option<u64>) -> Result<(), Diagnostic> {
    if doc
        .required_extensions
        .iter()
        .any(|e| e == crate::part_groups::EXTENSION)
    {
        crate::part_groups::decode(doc)
            .map_err(|e| e.into_iter().next().expect("decoder error"))?;
    }
    if !doc
        .required_extensions
        .iter()
        .any(|e| e == crate::tuplets::EXTENSION)
    {
        return Ok(());
    }
    let groups =
        crate::tuplets::decode(doc).map_err(|e| e.into_iter().next().expect("decoder error"))?;
    let endpoints: std::collections::BTreeSet<_> = groups
        .iter()
        .filter(|g| crate::tuplets::has_tuplets(g))
        .map(|g| g.event.0.as_str())
        .collect();
    for (_, event) in doc.events() {
        let host = event.id().is_some_and(|id| endpoints.contains(id));
        if !host && !event.attributes.contains_key("tm") {
            continue;
        }
        let fail = || {
            Diagnostic::error("practice.tuplet_clock_evidence","Tuplet admission requires exact measured duration = written type/dots times normal/actual ratio; missing or inconsistent evidence is not inferred",event.source)
        };
        let actual = event.duration.measured().ok_or_else(fail)?;
        let w = event
            .attributes
            .get("w")
            .and_then(Scalar::as_str)
            .ok_or_else(fail)?;
        let base: Rational = match w {
            "breve" => Rational::from_integer(8),
            "long" => Rational::from_integer(16),
            "maxima" => Rational::from_integer(32),
            _ => {
                let denominator: Rational = w.parse().map_err(|_| fail())?;
                if !denominator.is_positive() {
                    return Err(fail());
                }
                &Rational::from_integer(4) / &denominator
            }
        };
        let dots = event
            .attributes
            .get("dots")
            .and_then(Scalar::as_i64)
            .unwrap_or(0);
        if dots < 0 || dots as u64 > maximum_bits.unwrap_or(4096) {
            return Err(Diagnostic::error(
                "practice.tuplet_rational_budget",
                "Written dots exceed the selected timing arithmetic budget",
                event.source,
            ));
        }
        let mut written = base.clone();
        let mut addition = base;
        for _ in 0..dots {
            addition = &addition * &"1/2".parse::<Rational>().unwrap();
            written = &written + &addition;
        }
        let ratio = event
            .attributes
            .get("tm")
            .and_then(Scalar::as_str)
            .ok_or_else(fail)?;
        if !crate::registry::valid_ratio(ratio) {
            return Err(fail());
        }
        let (a, n) = ratio.split_once(':').ok_or_else(fail)?;
        let a: Rational = a.parse().map_err(|_| fail())?;
        let n: Rational = n.parse().map_err(|_| fail())?;
        if &(&written * &(&n / &a)) != actual {
            return Err(fail());
        }
    }
    Ok(())
}
