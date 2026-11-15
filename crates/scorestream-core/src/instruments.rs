//! Standard identities, not sound sources. Custom SFS instrument atoms remain valid.
//! Resolving an identity never changes pitch, transposition, clef or percussion maps.
use crate::notation::InstrumentId;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentFamily {
    Strings,
    Woodwinds,
    Brass,
    PluckedStrings,
    Keyboards,
    PitchedPercussion,
    UnpitchedPercussion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentPitchKind {
    Pitched,
    Unpitched,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentDefinition {
    pub id: InstrumentId,
    pub name: String,
    pub family: InstrumentFamily,
    pub pitch_kind: InstrumentPitchKind,
    pub aliases: Vec<InstrumentId>,
}

/// The checked-in catalog is shared with the generated desktop definitions.
/// It deliberately contains no MIDI programs, automatic transpositions or voices.
pub fn instrument_catalog() -> &'static [InstrumentDefinition] {
    static CATALOG: OnceLock<Vec<InstrumentDefinition>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("instruments/catalog.json"))
            .expect("the built-in instrument catalog must pass its contract tests")
    })
}

/// Exact, case-sensitive identifiers only. A missing match is a custom identity,
/// not an invalid score. The caller retains the original InstrumentId unchanged.
pub fn resolve_instrument(id: &InstrumentId) -> Option<&'static InstrumentDefinition> {
    instrument_catalog()
        .iter()
        .find(|definition| &definition.id == id || definition.aliases.contains(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn identities_and_aliases_are_unique_atoms_and_resolve_without_rewriting() {
        let mut seen = BTreeSet::new();
        for definition in instrument_catalog() {
            assert!(!definition.name.is_empty());
            for id in std::iter::once(&definition.id).chain(&definition.aliases) {
                assert!(id
                    .0
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b == b'_' || b == b'-'));
                assert!(!id.0.is_empty());
                assert!(seen.insert(id.clone()), "duplicate identity {id}");
                let original = id.clone();
                assert_eq!(resolve_instrument(id), Some(definition));
                assert_eq!(id, &original);
            }
        }
        for custom in ["unknown", "Piano", "flute.extra", "constructor", "__proto__"] {
            assert!(resolve_instrument(&InstrumentId(custom.into())).is_none());
        }
    }

    #[test]
    fn orchestral_families_include_beethoven_finale_and_explicit_pitch_kinds() {
        for id in [
            "violin", "viola", "cello", "double_bass", "flute", "oboe", "clarinet",
            "bassoon", "horn", "trumpet", "piccolo", "contrabassoon", "alto_trombone",
            "trombone", "bass_trombone", "timpani", "harp", "celesta", "xylophone", "crotales",
        ] {
            assert_eq!(
                resolve_instrument(&InstrumentId(id.into())).unwrap().pitch_kind,
                InstrumentPitchKind::Pitched
            );
        }
        for id in ["snare_drum", "bass_drum", "crash_cymbals", "triangle", "tam_tam"] {
            assert_eq!(
                resolve_instrument(&InstrumentId(id.into())).unwrap().pitch_kind,
                InstrumentPitchKind::Unpitched
            );
        }
    }
}
