//! Typed metadata codec; list-valued metadata does not extend generic Scalars.
#![allow(clippy::result_large_err)]
use crate::{lexer::Token, parser::raw_attributes};
use scorestream_core::*;
use std::{collections::BTreeMap, fmt::Write};

/// Metadata has its own typed boundary; arrays never enter generic Scalar fields.
pub(crate) fn read(
    metadata: &mut DocumentMetadata,
    tokens: &[Token<'_>],
    source: SourceRange,
) -> Result<(), Diagnostic> {
    if tokens.is_empty() {
        return Err(Diagnostic::error(
            "field_required",
            "Metadata record requires attributes",
            source,
        ));
    }
    let raw = raw_attributes(tokens, source)?;
    for (key, value) in raw {
        let token = tokens
            .iter()
            .find(|token| {
                token
                    .text
                    .split_once('=')
                    .is_some_and(|(name, _)| name == key)
            })
            .unwrap();
        let location = SourceRange {
            column: token.column,
            end_column: token.column + token.text.chars().count(),
            ..source
        };
        if metadata.source.contains_key(&key) {
            return Err(Diagnostic::error(
                "attribute_duplicate",
                format!("Repeated metadata field {key}"),
                location,
            ));
        }
        let ty = attribute_type("metadata", "", &key).ok_or_else(|| {
            Diagnostic::error(
                "field_unknown",
                format!("Unknown metadata field {key}"),
                location,
            )
        })?;
        if key == "authors" && ty == "string_list" {
            metadata.authors = serde_json::from_str::<Vec<String>>(&value).map_err(|_| {
                Diagnostic::error(
                    "metadata_authors_type",
                    "authors requires a JSON array of quoted names",
                    location,
                )
            })?;
        } else {
            let value = serde_json::from_str::<String>(&value).map_err(|_| {
                Diagnostic::error(
                    "string_invalid",
                    format!("{key} requires a valid JSON-quoted string"),
                    location,
                )
            })?;
            match key.as_str() {
                "title" => metadata.title = Some(value),
                "description" => metadata.description = Some(value),
                "header" => metadata.header = Some(value),
                "footer" => metadata.footer = Some(value),
                "subtitle" => metadata.subtitle = Some(value),
                "composer" => metadata.composer = Some(value),
                "lyricist" => metadata.lyricist = Some(value),
                "arranger" => metadata.arranger = Some(value),
                "work_number" => metadata.work_number = Some(value),
                "movement" => metadata.movement = Some(value),
                "movement_number" => metadata.movement_number = Some(value),
                "source_description" => metadata.source_description = Some(value),
                "rights" => metadata.rights = Some(value),
                "language" => metadata.language = Some(value),
                _ => {
                    return Err(Diagnostic::error(
                        "field_unknown",
                        format!("Unsupported metadata field {key}"),
                        location,
                    ))
                }
            }
        }
        metadata.source.insert(key, location);
    }
    Ok(())
}

pub(crate) fn write(metadata: &DocumentMetadata) -> String {
    let mut output = String::new();
    let mut fields: BTreeMap<&str, String> = metadata
        .text_fields()
        .into_iter()
        .filter_map(|(key, value)| {
            value.map(|text| (key, serde_json::to_string(text).expect("Unicode text")))
        })
        .collect();
    if !metadata.authors.is_empty() {
        fields.insert(
            "authors",
            serde_json::to_string(&metadata.authors).expect("Unicode names"),
        );
    }
    for (key, value) in fields {
        write!(output, " {key}={value}").unwrap();
    }
    output
}
