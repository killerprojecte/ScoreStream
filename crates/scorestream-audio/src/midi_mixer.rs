//! Explicit source-bound INITIAL MIDI intent for a local PCM synth, not device output.
use crate::{AudioError, Result, RouteAssignment, assignments::source_sha256};
use scorestream_codec::{
    Rational, Record, instrument_extension, instrument_metadata::InstrumentMetadata,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalMidiPolicy {
    /// Explicitly use the caller's bank-0 palette, require source program agreement,
    /// round percent CC7 to nearest integer (ties upward), and support only pan=0.
    InitialGmCc7CenteredV1,
}
#[derive(Clone, Debug, Serialize)]
pub struct VolumeReceipt {
    pub authored_percent: Rational,
    pub cc7: u8,
    pub realized_percent: Rational,
    /// Realized controller percentage minus authored percentage, not acoustic gain.
    pub quantization_error_percent: Rational,
}
#[derive(Clone, Debug, Serialize)]
pub struct RouteMidiReceipt {
    pub route: RouteAssignment,
    pub native_channel: u16,
    /// Unchanged source port/channel/name (including absence), never external output.
    pub authored_metadata: InstrumentMetadata,
    pub volume: Option<VolumeReceipt>,
    pub cc10: Option<u8>,
}
/// Only preparation from validated source can construct this token. It is not a
/// musical-performance certificate. A compiler wrapper must also match source SHA.
#[derive(Clone, Debug, Serialize)]
pub struct LocalMidiMix {
    source_sha256: String,
    policy: LocalMidiPolicy,
    external_addresses_realized: bool,
    routes: Vec<RouteMidiReceipt>,
}
impl LocalMidiMix {
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }
    pub fn routes(&self) -> &[RouteMidiReceipt] {
        &self.routes
    }
    pub fn verify_source(&self, source: &str) -> Result<()> {
        if source_sha256(source) != self.source_sha256 {
            return Err(invalid("Local MIDI mixer source revision mismatch"));
        }
        Ok(())
    }
    pub(crate) fn verify_routes(&self, routes: &[RouteAssignment]) -> Result<()> {
        if self.routes.len() != routes.len()
            || self.routes.iter().zip(routes).any(|(a, b)| a.route != *b)
        {
            return Err(invalid(
                "Local MIDI mixer routes differ from source-bound selected palette; technique expansions need separate qualification",
            ));
        }
        Ok(())
    }
}
fn invalid(s: impl Into<String>) -> AudioError {
    AudioError::InvalidPlan(s.into())
}
fn q(n: i64) -> Rational {
    Rational::from_integer(n)
}
fn volume(v: &Rational) -> Result<VolumeReceipt> {
    if v.is_negative() || v > &q(100) {
        return Err(invalid("MIDI volume percentage outside 0..100"));
    }
    let scaled = &(v * &q(127)) / &q(100);
    // Compare exact rational half-step thresholds; no float quantization.
    let mut cc = 0u8;
    for candidate in 1..=127u8 {
        let threshold = &q(2 * i64::from(candidate) - 1) / &q(2);
        if scaled >= threshold {
            cc = candidate;
        } else {
            break;
        }
    }
    let realized = &q(i64::from(cc) * 100) / &q(127);
    Ok(VolumeReceipt {
        authored_percent: v.clone(),
        cc7: cc,
        quantization_error_percent: &realized - v,
        realized_percent: realized,
    })
}
/// No default policy. All declared parts must have one exact selected local route.
/// Palette bank selection is caller-authored; source program alone implies no bank.
pub fn prepare_local_midi_mix(
    source: &str,
    expected_sha256: &str,
    routes: &[RouteAssignment],
    policy: LocalMidiPolicy,
) -> Result<LocalMidiMix> {
    let hash = source_sha256(source);
    if hash != expected_sha256 {
        return Err(invalid("Expected MIDI mixer source SHA differs"));
    }
    let doc =
        scorestream_codec::parse(source).map_err(|e| invalid(format!("Invalid source: {e:?}")))?;
    let mut metadata = BTreeMap::new();
    for frame in &doc.frames {
        for record in &frame.records {
            if let Record::Mark(mark) = record {
                if mark.kind == instrument_extension::KIND {
                    if frame.time != Rational::zero() {
                        return Err(invalid("Only initial authored MIDI metadata is realized"));
                    }
                    let value =
                        instrument_extension::decode(mark).map_err(|e| invalid(e.to_string()))?;
                    if metadata.insert(mark.target.clone(), value).is_some() {
                        return Err(invalid("Duplicate instrument metadata"));
                    }
                }
            }
        }
    }
    if routes.len() != doc.parts.len() || routes.len() > 256 {
        return Err(invalid(
            "Require one route for every declared part within native channel capacity",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut receipts = Vec::new();
    for (index, route) in routes.iter().enumerate() {
        let part = doc
            .parts
            .iter()
            .find(|p| p.id == route.part)
            .ok_or_else(|| invalid("Unknown local route part"))?;
        if !seen.insert(&route.part)
            || part.instrument != route.instrument
            || route.bank != 0
            || route.program > 127
        {
            return Err(invalid(
                "Local route is duplicate, mismatched, or not explicit bank 0 GM palette",
            ));
        }
        let authored = metadata
            .remove(&route.part)
            .ok_or_else(|| invalid("Missing initial MIDI source metadata"))?;
        if authored
            .midi_device
            .as_ref()
            .is_some_and(|d| !d.name.is_empty())
        {
            return Err(invalid(
                "Named external MIDI device is not realized by local PCM",
            ));
        }
        let midi = authored
            .midi_instrument
            .as_ref()
            .ok_or_else(|| invalid("Missing authored MIDI assignment"))?;
        if midi.program.map(u16::from) != Some(u16::from(route.program) + 1) {
            return Err(invalid(
                "Authored MIDI program differs from explicitly selected local preset",
            ));
        }
        if midi.pan.as_ref().is_some_and(|p| *p != Rational::zero()) {
            return Err(invalid(
                "Only exact authored centered pan is supported; 3D/noncentral pan is not inferred",
            ));
        }
        let volume = midi.volume.as_ref().map(volume).transpose()?;
        let cc10 = midi.pan.as_ref().map(|_| 64);
        receipts.push(RouteMidiReceipt {
            route: route.clone(),
            native_channel: index as u16,
            authored_metadata: authored,
            volume,
            cc10,
        });
    }
    Ok(LocalMidiMix {
        source_sha256: hash,
        policy,
        external_addresses_realized: false,
        routes: receipts,
    })
}
