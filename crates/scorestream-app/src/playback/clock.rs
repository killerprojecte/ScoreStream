//! One clock for markers, note gates and both expression implementations. Exact
//! fermata seconds are converted only at the existing PlaybackPlan boundary.
use super::tempo::TempoMap;
use scorestream_codec::{Rational, fermata_timing::TimingProjection, notation::ResolvedScore};

pub(super) enum PlaybackClock<'a> {
    Ordinary(TempoMap),
    Explicit {
        timing: &'a TimingProjection,
        boundaries: Vec<Rational>,
    },
}
impl<'a> PlaybackClock<'a> {
    pub fn new(
        score: &ResolvedScore,
        timing: Option<&'a TimingProjection>,
    ) -> Result<Self, String> {
        if let Some(timing) = timing {
            for hold in timing.holds() {
                let start = hold.seconds_start.to_f64();
                let end = hold.seconds_end.to_f64();
                if !start.is_finite() || !end.is_finite() || end <= start {
                    return Err(format!(
                        "fermata.clock_precision: Positive exact hold {} collapses at the audio clock boundary",
                        hold.id
                    ));
                }
            }
            let mut boundaries: Vec<_> = score
                .tempos
                .iter()
                .map(|t| t.at.clone())
                .chain(timing.holds().iter().map(|h| h.boundary.clone()))
                .collect();
            boundaries.sort();
            boundaries.dedup();
            Ok(Self::Explicit { timing, boundaries })
        } else {
            Ok(Self::Ordinary(TempoMap::new(score)))
        }
    }
    pub fn seconds(&self, at: &Rational) -> Result<f64, String> {
        match self {
            Self::Ordinary(tempo) => tempo.seconds(at),
            Self::Explicit { timing, .. } => {
                // TimingProjection intentionally clamps seek/marker queries. A
                // compiler release outside its domain must never clamp silently.
                if at.is_negative() || at > timing.quarter_end() {
                    return Err("fermata.clock_range: Playback query/release lies outside the exact performed timeline".into());
                }
                let seconds = timing.after(at).to_f64();
                if seconds.is_finite() && (0.0..=21_600.).contains(&seconds) {
                    Ok(seconds)
                } else {
                    Err("Playback timeline must fit in six hours at the specified tempo".into())
                }
            }
        }
    }
    pub fn boundaries<'b>(&'b self, start: &Rational, end: &Rational) -> Vec<&'b Rational> {
        match self {
            Self::Ordinary(tempo) => tempo.boundaries(start, end).collect(),
            Self::Explicit { boundaries, .. } => {
                let lo = boundaries.partition_point(|q| q <= start);
                let hi = boundaries.partition_point(|q| q < end);
                boundaries[lo..hi].iter().collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::{
        FermataSoundPolicy, discover_source_fermatas, plan_source_with_explicit_fermatas,
    };
    use scorestream_codec::fermata_timing::{ExplicitFermataPolicy, ExplicitHoldGroup};

    #[test]
    fn explicit_audio_queries_reject_instead_of_clamping_outside_the_clock() {
        let source = "!score 1.1 unit=q\n!requires scorestream.fermata@1\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m len=1\nv C4:1 id=a\n+ #a scorestream.fermata shape=normal orientation=upright\n.\n!end at=1\n";
        let occurrence = discover_source_fermatas(source)
            .unwrap()
            .occurrences
            .remove(0);
        let result = plan_source_with_explicit_fermatas(
            source,
            &ExplicitFermataPolicy {
                groups: vec![ExplicitHoldGroup {
                    id: "hold".into(),
                    members: vec![occurrence.id],
                    added_seconds: Rational::one(),
                }],
            },
            FermataSoundPolicy::SustainSelectedAnchors,
        )
        .unwrap();
        let clock = PlaybackClock::Explicit {
            timing: &result.timing,
            boundaries: vec![],
        };
        assert!(
            clock
                .seconds(&Rational::from_integer(-1))
                .unwrap_err()
                .contains("fermata.clock_range")
        );
        assert!(
            clock
                .seconds(&Rational::from_integer(2))
                .unwrap_err()
                .contains("fermata.clock_range")
        );
        assert_eq!(clock.seconds(&Rational::one()).unwrap(), 1.5);
        // The metadata API intentionally retains its documented clamp contract.
        assert_eq!(
            result.timing.after(&Rational::from_integer(2)).to_f64(),
            1.5
        );
    }
}
