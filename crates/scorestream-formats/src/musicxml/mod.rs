//! Small strict MusicXML 4.0 score-partwise profile; not complete MusicXML support.
mod read;
mod ties;
mod write;
mod xml;
use crate::{Conversion, MappingReport, ScoreAdapter};
pub use read::{read, read_reader};
use scorestream_core::score::{NoteType, ScoreDocument};
pub use write::write;
pub use xml::{ScanSummary, scan};
#[derive(Clone, Copy, Debug, Default)]
pub struct MusicXml;
impl ScoreAdapter for MusicXml {
    fn read(&self, source: &str) -> Result<Conversion<ScoreDocument>, MappingReport> {
        read(source)
    }
    fn write(&self, score: &ScoreDocument) -> Result<Conversion<String>, MappingReport> {
        write(score)
    }
}
pub(crate) fn note_type(value: &str) -> Option<NoteType> {
    Some(match value {
        "maxima" => NoteType::Maxima,
        "long" => NoteType::Long,
        "breve" => NoteType::Breve,
        "whole" => NoteType::Whole,
        "half" => NoteType::Half,
        "quarter" => NoteType::Quarter,
        "eighth" => NoteType::Eighth,
        "16th" => NoteType::N16,
        "32nd" => NoteType::N32,
        "64th" => NoteType::N64,
        "128th" => NoteType::N128,
        "256th" => NoteType::N256,
        "512th" => NoteType::N512,
        "1024th" => NoteType::N1024,
        _ => return None,
    })
}
pub(crate) fn type_name(value: NoteType) -> &'static str {
    match value {
        NoteType::Maxima => "maxima",
        NoteType::Long => "long",
        NoteType::Breve => "breve",
        NoteType::Whole => "whole",
        NoteType::Half => "half",
        NoteType::Quarter => "quarter",
        NoteType::Eighth => "eighth",
        NoteType::N16 => "16th",
        NoteType::N32 => "32nd",
        NoteType::N64 => "64th",
        NoteType::N128 => "128th",
        NoteType::N256 => "256th",
        NoteType::N512 => "512th",
        NoteType::N1024 => "1024th",
    }
}
