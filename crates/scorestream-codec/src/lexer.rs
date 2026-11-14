// Tokenization passes a single structured diagnostic straight to the parser.
#![allow(clippy::result_large_err)]
use scorestream_core::{Diagnostic, SourceRange};
#[derive(Debug)]
pub(crate) struct Token<'a> {
    pub text: &'a str,
    pub column: usize,
}
/// Split only at unquoted horizontal whitespace. `#` is never a comment.
pub(crate) fn tokenize(line: &str, line_number: usize) -> Result<Vec<Token<'_>>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut start = None;
    let mut quote = false;
    let mut escaped = false;
    let mut authors_array = false;
    let metadata = line.trim_start_matches([' ', '\t']).starts_with("!meta ")
        || line.trim_start_matches([' ', '\t']).starts_with("!meta\t");
    for (offset, c) in line.char_indices() {
        if c.is_control() && c != '\t' {
            return Err(Diagnostic::error(
                "lexical_control",
                "Control character in source record",
                SourceRange::line(line_number, line.chars().count()),
            ));
        }
        if !quote && !authors_array && (c == ' ' || c == '\t') {
            if let Some(begin) = start.take() {
                tokens.push(Token {
                    text: &line[begin..offset],
                    column: line[..begin].chars().count() + 1,
                });
            }
            continue;
        }
        if !quote && c.is_whitespace() && !(authors_array && matches!(c, ' ' | '\t')) {
            return Err(Diagnostic::error(
                "lexical_whitespace",
                "Use spaces or tabs between fields",
                SourceRange::line(line_number, line.chars().count()),
            ));
        }
        start.get_or_insert(offset);
        if !quote
            && metadata
            && c == '['
            && start.is_some_and(|begin| &line[begin..offset] == "authors=")
        {
            authors_array = true;
        } else if !quote && c == ']' {
            authors_array = false;
        }
        if quote && escaped {
            escaped = false;
        } else if quote && c == '\\' {
            escaped = true;
        } else if c == '"' {
            quote = !quote;
        }
    }
    if quote {
        return Err(Diagnostic::error(
            "string_unclosed",
            "Unclosed JSON string; physical newlines are not allowed",
            SourceRange::line(line_number, line.chars().count()),
        ));
    }
    if authors_array {
        return Err(Diagnostic::error(
            "metadata_authors_type",
            "Unclosed authors array",
            SourceRange::line(line_number, line.chars().count()),
        ));
    }
    if let Some(begin) = start {
        tokens.push(Token {
            text: &line[begin..],
            column: line[..begin].chars().count() + 1,
        });
    }
    Ok(tokens)
}
