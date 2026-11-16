//! Intrinsic score text, distinct from files, notation directions and markup.
use crate::{Diagnostic, Severity, SourceRange};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

/// Shared typed metadata for SFS and format adapters. Author order is meaningful.
/// Consumers must render text as text (escaping their output format), never markup.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentMetadata {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub title: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub description: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub header: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub footer: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub subtitle: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub composer: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub lyricist: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub arranger: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub work_number: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub movement: Option<String>,
    /// Authored movement identifier, independent of its title.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub movement_number: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub rights: Option<String>,
    /// Authored score-source description, not a path or authority to fetch a resource.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub source_description: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_text"
    )]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
    /// Field-level SFS source provenance, intentionally not serialized in the AST.
    #[serde(skip)]
    pub source: BTreeMap<String, SourceRange>,
}

// A missing field is optional; an explicitly supplied JSON null is not text.
fn optional_text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}

impl DocumentMetadata {
    pub fn is_empty(&self) -> bool {
        self.authors.is_empty() && self.text_fields().iter().all(|(_, value)| value.is_none())
    }
    /// Named text fields in stable canonical order. Authors are serialized separately.
    pub fn text_fields(&self) -> [(&'static str, Option<&str>); 14] {
        [
            ("arranger", self.arranger.as_deref()),
            ("composer", self.composer.as_deref()),
            ("description", self.description.as_deref()),
            ("footer", self.footer.as_deref()),
            ("header", self.header.as_deref()),
            ("language", self.language.as_deref()),
            ("lyricist", self.lyricist.as_deref()),
            ("movement", self.movement.as_deref()),
            ("movement_number", self.movement_number.as_deref()),
            ("rights", self.rights.as_deref()),
            ("source_description", self.source_description.as_deref()),
            ("subtitle", self.subtitle.as_deref()),
            ("title", self.title.as_deref()),
            ("work_number", self.work_number.as_deref()),
        ]
    }
    pub fn validate(&self, fallback: SourceRange) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for (field, value) in self.text_fields() {
            if value.is_some_and(|value| value.chars().any(invalid_text_character)) {
                let mut diagnostic = self.error(
                    field,
                    "metadata_text",
                    "Preserved score text contains control characters; a renderer/exporter must report if it cannot represent them",
                    fallback,
                );
                // All previously valid JSON text remains readable in development 1.1.
                // In particular, do not erase an imported character just to fit XML.
                diagnostic.severity = Severity::Warning;
                diagnostics.push(diagnostic);
            }
        }
        for (index, author) in self.authors.iter().enumerate() {
            if author.trim().is_empty()
                || author
                    .chars()
                    .any(|c| c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
            {
                diagnostics.push(self.error("authors", "metadata_author", format!("Author {} requires a nonblank, single-line name without control characters", index + 1), fallback));
            }
        }
        diagnostics
    }
    fn error(
        &self,
        field: &str,
        code: &str,
        message: impl Into<String>,
        fallback: SourceRange,
    ) -> Diagnostic {
        let mut diagnostic = Diagnostic::error(
            code,
            message,
            self.source.get(field).copied().unwrap_or(fallback),
        );
        diagnostic.object_id = Some(format!("metadata.{field}"));
        diagnostic
    }
}

fn invalid_text_character(c: char) -> bool {
    c.is_control() && !matches!(c, '\t' | '\n' | '\r')
}
