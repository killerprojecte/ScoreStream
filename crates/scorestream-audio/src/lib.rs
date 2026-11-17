//! Native offline PCM, not a MIDI-device or VST3 host. Musical interpretation
//! belongs to scorestream-app. The source-bound assignment layer calls that real
//! compiler without rewriting source or inventing a fallback patch. Unsupported
//! performance features fail before any PCM output.
#![deny(unsafe_op_in_unsafe_fn)]

pub mod assignments;
#[cfg(target_os = "linux")]
pub mod audition_session;
#[cfg(target_os = "linux")]
pub mod policy_draft;
pub mod attack_rehearsal;
pub mod dry_lanes;
#[cfg(target_os = "linux")]
pub mod dry_admission;
pub mod midi_mixer;
#[cfg(target_os = "linux")]
pub mod pizzicato;
#[cfg(target_os = "linux")]
mod native;
mod plan;
#[cfg(target_os = "linux")]
mod render;
mod wav;
pub use wav::{PendingWav, PublishedWav};

use scorestream_app::playback::{PerformanceDiagnostic, PlaybackPlan};
use serde::{Deserialize, Serialize};
use std::{
    fmt, io,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

#[cfg(target_os = "linux")]
pub use render::FluidSynthBackend;

pub type Result<T> = std::result::Result<T, AudioError>;

#[derive(Debug)]
pub enum AudioError {
    Unavailable(String),
    InvalidPlan(String),
    Unsupported(String),
    Native(String),
    Cancelled,
    Clipping { frame: u64, peak: f32 },
    PolyphonyLimit { voices: u32, limit: u32 },
    Io(io::Error),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(s) => write!(f, "Audio backend unavailable: {s}"),
            Self::InvalidPlan(s) => write!(f, "Invalid audio plan: {s}"),
            Self::Unsupported(s) => write!(f, "Unsupported audio feature: {s}"),
            Self::Native(s) => write!(f, "Native audio failure: {s}"),
            Self::Cancelled => write!(f, "Audio rendering cancelled"),
            Self::Clipping { frame, peak } => write!(
                f,
                "PCM would clip at frame {frame} (peak {peak}); reduce gain"
            ),
            Self::PolyphonyLimit { voices, limit } => write!(
                f,
                "Native voice count {voices} reached limit {limit}; raise the explicit voice budget"
            ),
            Self::Io(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for AudioError {}
impl From<io::Error> for AudioError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// An explicit local playback assignment, separate from score instrument IDs.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RouteAssignment {
    pub part: String,
    pub instrument: String,
    pub bank: u16,
    /// Zero based, 0..=127. Never inferred from a name or catalog position.
    pub program: u8,
}

#[derive(Clone, Debug)]
pub struct RenderOptions {
    pub sample_rate: u32,
    pub block_frames: usize,
    pub gain: f64,
    pub tail_seconds: f64,
    pub polyphony: u32,
    pub max_notes: usize,
    /// Bound the bank input, not native resident memory. Only one copy is loaded.
    pub max_soundfont_bytes: u64,
    /// Optional caller assertion. Hash is checked before native parsing; preset
    /// names are checked after loading the verified bank, before any PCM.
    /// Source-qualified sample assignments always populate this assertion.
    pub bank_verification: Option<BankVerification>,
    /// Explicit source-bound initial mixer interpretation; never enabled by default.
    pub local_midi_mix: Option<midi_mixer::LocalMidiMix>,
    pub attack_only_rehearsal: Option<attack_rehearsal::AttackOnlyRehearsal>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BankVerification {
    pub sha256: String,
    /// Exact native preset names in route order; no aliases or display-name match.
    pub preset_names: Vec<String>,
}
impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            block_frames: 64,
            gain: 0.04,
            tail_seconds: 2.0,
            polyphony: 4096,
            max_notes: 1_000_000,
            max_soundfont_bytes: 2 * 1024 * 1024 * 1024,
            bank_verification: None,
            local_midi_mix: None,
            attack_only_rehearsal: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BackendCapabilities {
    pub id: String,
    pub version: String,
    pub library_path: PathBuf,
    /// The checked file on disk, not an attestation of resident code bytes.
    /// The OS may reuse a module already loaded earlier in this process.
    pub checked_library_sha256: String,
    pub offline_pcm: bool,
    pub realtime_audio: bool,
    pub midi_device_output: bool,
    pub vst3: bool,
    pub per_note_expression: bool,
    pub microtonal_pitch: bool,
    pub max_routes: u16,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderedRoute {
    pub assignment: RouteAssignment,
    pub native_channel: u16,
    pub preset_name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct RenderReport {
    pub backend: BackendCapabilities,
    pub soundfont_path: PathBuf,
    pub soundfont_sha256: String,
    pub sample_rate: u32,
    pub score_frames: u64,
    pub frames: u64,
    pub note_count: usize,
    pub routes: Vec<RenderedRoute>,
    pub local_midi_mix: Option<midi_mixer::LocalMidiMix>,
    pub attack_only_rehearsal: Option<attack_rehearsal::AttackOnlyRehearsal>,
    pub internal_quantum_frames: u32,
    pub block_frames: usize,
    pub gain: f64,
    pub native_polyphony_limit: u32,
    pub peak_native_voices: u32,
    pub noteoffs_without_live_voice: u64,
    pub peak: f32,
    pub rms: f64,
    pub final_50ms_peak: f32,
    pub tail_may_be_truncated: bool,
    pub warnings: Vec<String>,
    pub diagnostics: Vec<PerformanceDiagnostic>,
}

/// PCM blocks are interleaved stereo f32. A failing/cancelled stream may have
/// emitted partial data. The consumer MUST keep that data private and discard it.
/// `render_wav` provides this transaction for a new local file.
pub trait PcmSink {
    fn write(&mut self, start_frame: u64, samples: &[f32]) -> Result<()>;
}
impl<F: FnMut(u64, &[f32]) -> Result<()>> PcmSink for F {
    fn write(&mut self, start_frame: u64, samples: &[f32]) -> Result<()> {
        self(start_frame, samples)
    }
}

/// Validate semantics and capacity without loading a library or bank.
pub fn validate_plan(
    plan: &PlaybackPlan,
    routes: &[RouteAssignment],
    options: &RenderOptions,
) -> Result<()> {
    plan::Prepared::new(plan, routes, options).map(|_| ())
}

/// Discover an existing standard bank, without installing or copying it.
pub fn default_soundfont() -> Result<PathBuf> {
    for path in [
        "/usr/share/sounds/sf2/default-GM.sf2",
        "/usr/share/soundfonts/default.sf2",
    ] {
        let path = Path::new(path);
        if path.is_file() {
            return Ok(path.canonicalize()?);
        }
    }
    Err(AudioError::Unavailable(
        "No local default bank; select an existing authorized SF2".into(),
    ))
}

#[cfg(not(target_os = "linux"))]
pub struct FluidSynthBackend {
    capabilities: BackendCapabilities,
}
#[cfg(not(target_os = "linux"))]
impl FluidSynthBackend {
    pub fn capabilities(&self) -> &BackendCapabilities {
        &self.capabilities
    }
    pub fn open_installed() -> Result<Self> {
        Err(AudioError::Unavailable(
            "This native adapter currently supports Linux only".into(),
        ))
    }
    pub fn render_pcm(
        &self,
        _plan: &PlaybackPlan,
        _routes: &[RouteAssignment],
        _bank: &Path,
        _options: &RenderOptions,
        _cancel: &AtomicBool,
        _sink: &mut impl PcmSink,
    ) -> Result<RenderReport> {
        Err(AudioError::Unavailable(
            "This native adapter currently supports Linux only".into(),
        ))
    }
    pub fn render_wav(
        &self,
        _plan: &PlaybackPlan,
        _routes: &[RouteAssignment],
        _bank: &Path,
        _options: &RenderOptions,
        _cancel: &AtomicBool,
        _output: &Path,
    ) -> Result<RenderReport> {
        Err(AudioError::Unavailable(
            "This native adapter currently supports Linux only".into(),
        ))
    }
}

/// Create a new PCM16 stereo WAV atomically; never overwrite an existing file.
pub fn render_wav(
    backend: &FluidSynthBackend,
    plan: &PlaybackPlan,
    routes: &[RouteAssignment],
    bank: &Path,
    options: &RenderOptions,
    cancel: &AtomicBool,
    output: &Path,
) -> Result<RenderReport> {
    wav::render(backend, plan, routes, bank, options, cancel, output)
}
