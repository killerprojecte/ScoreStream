//! Explicit interpretation of retained authored staccato/accent data. No rewritten
//! source, flattened archival payload or implicit performance fallback is exposed.
use super::{lower::LoweringProfile, perform, *};
use crate::{Diagnostic, Document, Pitch, Severity, articulation_extension, articulation_model::*};
use std::collections::{BTreeMap, BTreeSet};

/// A performer choice, not a duration or loudness supplied by MusicXML.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExplicitArticulationPolicy {
    /// Existing simple engine: staccato gate 1/2; accent +16 velocity steps.
    /// Numeric sound dynamics remain authoritative and cannot combine with accents.
    SimpleStaccatoAccentV1,
}
#[derive(Clone, Debug)]
pub struct QualifiedArticulationPerformance {
    source: Document,
    performed: PerformedScore,
    authored: AuthoredArticulations,
    policy: ExplicitArticulationPolicy,
}
impl QualifiedArticulationPerformance {
    pub fn source(&self) -> &Document {
        &self.source
    }
    pub fn performed(&self) -> &PerformedScore {
        &self.performed
    }
    pub fn authored(&self) -> &AuthoredArticulations {
        &self.authored
    }
    pub fn policy(&self) -> ExplicitArticulationPolicy {
        self.policy
    }
}
fn fail(doc: &Document, code: &str, message: &str) -> Diagnostic {
    Diagnostic::error(code, message, doc.source)
}
pub fn perform_document_with_explicit_articulations(
    doc: &Document,
    policy: ExplicitArticulationPolicy,
    limits: PerformanceLimits,
) -> Result<QualifiedArticulationPerformance, Vec<Diagnostic>> {
    let errors: Vec<_> = crate::validate(doc, None)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    let authored = articulation_extension::decode(doc)?.ok_or_else(|| {
        vec![fail(
            doc,
            "articulation.policy_source",
            "This explicit profile requires retained authored articulation data",
        )]
    })?;
    let performed = perform::perform(
        doc,
        limits,
        LoweringProfile::ExplicitArticulations,
        None,
        None,
    )
    .map_err(|e| vec![e])?;
    // A sign at a tied continuation cannot invent a new attack, and an interior
    // staccato cannot be silently discarded by the chain's terminal gate.
    let mut tied_sources = BTreeSet::new();
    let mut tied_targets = BTreeSet::new();
    for relation in &performed.score.relations {
        if matches!(relation.kind, RelationKind::Tie { sound: true, .. }) {
            for pair in relation.anchors.windows(2) {
                tied_sources.insert(&pair[0].event);
                tied_targets.insert(&pair[1].event);
            }
        }
    }
    let written: BTreeSet<_> = authored.events.iter().map(|event| &event.event).collect();
    let occurrences: BTreeSet<_> = performed
        .events
        .iter()
        .filter(|receipt| written.contains(&receipt.written_id))
        .map(|receipt| &receipt.id)
        .collect();
    for direction in &performed.score.directions {
        if let (Scope::Event(id), DirectionKind::Articulation { value }) =
            (&direction.scope, &direction.kind)
        {
            if !occurrences.contains(id) {
                continue;
            }
            if (*value == Articulation::Staccato && tied_sources.contains(id))
                || (*value == Articulation::Accent && tied_targets.contains(id))
            {
                return Err(vec![fail(
                    doc,
                    "articulation.tie_interpretation",
                    "Interior staccato or continuation accent needs a different explicit tie articulation interpretation",
                )]);
            }
        }
    }
    Ok(QualifiedArticulationPerformance {
        source: doc.clone(),
        performed,
        authored,
        policy,
    })
}

pub(super) fn append_directions(
    doc: &Document,
    score: &mut ResolvedScore,
) -> Result<(), Diagnostic> {
    let authored = articulation_extension::decode(doc)
        .map_err(|mut errors| errors.remove(0))?
        .ok_or_else(|| {
            fail(
                doc,
                "articulation.policy_source",
                "Missing authored articulation data",
            )
        })?;
    let events: BTreeMap<_, _> = score
        .events
        .iter()
        .map(|event| (&event.id, event))
        .collect();
    for entry in authored.events {
        let event = events.get(&entry.event).ok_or_else(|| {
            fail(
                doc,
                "articulation.anchor",
                "Authored articulation anchor is absent",
            )
        })?;
        if event.role != EventRole::Note || event.written_pitch == Pitch::Rest {
            return Err(fail(
                doc,
                "articulation.anchor_interpretation",
                "This profile realizes measured sounding note articulations only",
            ));
        }
        let mut seen = BTreeSet::new();
        for wrapper in &entry.notations {
            for group in &wrapper.groups {
                for item in &group.items {
                    let value = match item.sign {
                        ArticulationSign::Staccato {} => Articulation::Staccato,
                        ArticulationSign::Accent {} => Articulation::Accent,
                        _ => {
                            return Err(fail(
                                doc,
                                "articulation.sign_interpretation",
                                "Selected explicit profile realizes staccato and accent only; other signs remain authored data",
                            ));
                        }
                    };
                    if !seen.insert(value) {
                        return Err(fail(
                            doc,
                            "articulation.stacked_interpretation",
                            "Repeated articulation signs need an explicit stacking interpretation",
                        ));
                    }
                    score.directions.push(Direction {
                        at: event.start.clone(),
                        end: None,
                        scope: Scope::Event(event.id.clone()),
                        kind: DirectionKind::Articulation { value },
                        placement: item.placement,
                        origin: Origin {
                            object_id: format!(
                                "explicit-articulation:{}:{}",
                                event.id,
                                item.sign.name()
                            ),
                            source: event.origin.source,
                        },
                    });
                }
            }
        }
    }
    Ok(())
}
