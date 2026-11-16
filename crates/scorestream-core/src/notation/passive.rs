//! Individually audited, closed printed/provenance receipts. No sound families.
use crate::{Diagnostic, Document};
pub(super) fn is_passive(e: &str) -> bool {
    matches!(
        e,
        "scorestream.provenance@1"
            | "scorestream.metronome_display@1"
            | "scorestream.part_labels@1"
            | "scorestream.defaults@1"
            | "scorestream.credits@1"
            | "scorestream.lyrics@1"
            | "scorestream.print@1"
            | "scorestream.geometry@1"
    )
}
pub(super) fn validate(doc: &Document) -> Result<(), Diagnostic> {
    for e in &doc.required_extensions {
        let result = match e.as_str() {
            crate::provenance::EXTENSION => crate::provenance::decode(doc).map(|_| ()),
            crate::metronome_display::EXTENSION => {
                crate::metronome_display::decode(doc).map(|_| ())
            }
            crate::part_labels::EXTENSION => crate::part_labels::decode(doc).map(|_| ()),
            crate::defaults::EXTENSION => crate::defaults::decode(doc).map(|_| ()),
            crate::credits::EXTENSION => crate::credits::decode(doc).map(|_| ()),
            crate::lyrics::EXTENSION => crate::lyrics::decode(doc).map(|_| ()),
            crate::print_layout::EXTENSION => crate::print_layout::decode(doc).map(|_| ()),
            crate::geometry::EXTENSION => crate::geometry::decode(doc).map(|_| ()),
            _ => continue,
        };
        result.map_err(|errors| errors.into_iter().next().expect("typed decoder error"))?;
    }
    Ok(())
}
pub(super) fn pending(e: &str) -> bool {
    matches!(
        e,
        "scorestream.wedge@1"
            | "scorestream.dashes@1"
            | "scorestream.slur@1"
            | "scorestream.ornaments@1"
    )
}
