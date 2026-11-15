//! Format-neutral typed notation. No audio velocities or output-device policy.
use crate::{Duration, Pitch, Rational, SourceRange};
use serde::{Deserialize, Serialize};

macro_rules! identity {
    ($($name:ident),+ $(,)?) => {$ (
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);
        impl std::fmt::Display for $name { fn fmt(&self, f:&mut std::fmt::Formatter<'_>)->std::fmt::Result { self.0.fmt(f) } }
    )+};
}
identity!(PartId, VoiceId, EventId, RelationId, InstrumentId);
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StaffId {
    pub part: PartId,
    pub number: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum Scope {
    Score,
    Part(PartId),
    Staff(StaffId),
    Voice(VoiceId),
    Event(EventId),
}
impl Scope {
    pub fn rank(&self) -> u8 {
        match self {
            Self::Score => 0,
            Self::Part(_) => 1,
            Self::Staff(_) => 2,
            Self::Voice(_) => 3,
            Self::Event(_) => 4,
        }
    }
    pub fn applies(&self, event: &ResolvedEvent) -> bool {
        match self {
            Self::Score => true,
            Self::Part(id) => *id == event.part,
            Self::Staff(id) => *id == event.staff,
            Self::Voice(id) => *id == event.voice,
            Self::Event(id) => *id == event.id,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    pub object_id: String,
    pub source: SourceRange,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DynamicLevel {
    Pppp,
    Ppp,
    Pp,
    P,
    Mp,
    Mf,
    F,
    Ff,
    Fff,
    Ffff,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "level", rename_all = "snake_case")]
pub enum DynamicSign {
    Level(DynamicLevel),
    Sforzando,
    FortePiano,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Articulation {
    Staccato,
    Staccatissimo,
    Tenuto,
    Accent,
    StrongAccent,
    BreathMark,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Placement {
    Above,
    Below,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BeatUnit {
    Whole,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Beat {
    pub unit: BeatUnit,
    pub dotted: bool,
}
impl Beat {
    pub fn quarters(&self) -> Rational {
        let base = match self.unit {
            BeatUnit::Whole => "4",
            BeatUnit::Half => "2",
            BeatUnit::Quarter => "1",
            BeatUnit::Eighth => "1/2",
            BeatUnit::Sixteenth => "1/4",
        }
        .parse::<Rational>()
        .unwrap();
        if self.dotted {
            &base * &"3/2".parse().unwrap()
        } else {
            base
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tempo {
    pub beat: Beat,
    pub bpm: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TempoChange {
    pub at: Rational,
    pub value: Tempo,
    pub origin: Origin,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DirectionKind {
    Dynamic {
        sign: DynamicSign,
    },
    Articulation {
        value: Articulation,
    },
    Words {
        text: String,
        language: Option<String>,
    },
    Metronome {
        tempo: Tempo,
        text: Option<String>,
    },
    Accidental {
        value: String,
        visible: Option<bool>,
        cautionary: bool,
        parentheses: bool,
        editorial: bool,
    },
    Lyric {
        verse: u64,
        text: String,
        syllabic: Option<String>,
        language: Option<String>,
    },
    Harmony {
        root: String,
        quality: String,
        bass: Option<String>,
        text: Option<String>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Direction {
    pub at: Rational,
    /// Explicit finite scope in the shared model; SFS 1.1 does not add syntax.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<Rational>,
    pub scope: Scope,
    pub kind: DirectionKind,
    pub placement: Option<Placement>,
    pub origin: Origin,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventRole {
    Note,
    Cue,
    Grace,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraceNotation {
    pub anchor: EventId,
    pub order: i64,
    pub slash: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedEvent {
    pub id: EventId,
    pub voice: VoiceId,
    pub part: PartId,
    pub staff: StaffId,
    pub instrument: InstrumentId,
    pub start: Rational,
    pub duration: Duration,
    pub written_pitch: Pitch,
    pub sounding_pitch: Option<Rational>,
    pub role: EventRole,
    pub grace: Option<GraceNotation>,
    pub origin: Origin,
}
impl ResolvedEvent {
    pub fn end(&self) -> Rational {
        self.duration
            .measured()
            .map(|d| &self.start + d)
            .unwrap_or_else(|| self.start.clone())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HairpinKind {
    Crescendo,
    Diminuendo,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RelationKind {
    Tie {
        sound: bool,
        print: bool,
    },
    Slur,
    Hairpin {
        direction: HairpinKind,
    },
    Pedal,
    Octave {
        octaves: i64,
        sound: bool,
        print: bool,
    },
    Tuplet {
        actual: u64,
        normal: u64,
    },
    Beam {
        level: u64,
    },
    LyricExtend {
        verse: u64,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationAnchor {
    pub event: EventId,
    pub at: Rational,
    pub pedal_value: Option<Rational>,
    pub origin: Origin,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRelation {
    pub id: RelationId,
    pub scope: Scope,
    pub kind: RelationKind,
    pub anchors: Vec<RelationAnchor>,
    pub origin: Origin,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedScore {
    pub end: Rational,
    pub events: Vec<ResolvedEvent>,
    pub directions: Vec<Direction>,
    pub relations: Vec<ResolvedRelation>,
    pub tempos: Vec<TempoChange>,
}
