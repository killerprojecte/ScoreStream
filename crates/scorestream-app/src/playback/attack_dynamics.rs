//! Note-local explicit NoteOn values. No scope assignment or inferred trajectory.
use super::EnvelopePoint;
use scorestream_codec::{Rational, notation::ResolvedEvent};
/// Return a fixed exact-effective velocity for a static underlying envelope.
/// Changing expression needs a separately specified interaction policy.
pub(super) fn apply(
    event: &ResolvedEvent,
    envelope: &[EnvelopePoint],
    duration: f64,
) -> Result<Option<(u8, Vec<EnvelopePoint>)>, String> {
    apply_with_policy(event, envelope, duration, None)
}
pub(super) fn apply_with_policy(
    event: &ResolvedEvent,
    envelope: &[EnvelopePoint],
    duration: f64,
    policy: Option<super::PracticeNoteExpressionPolicy>,
) -> Result<Option<(u8, Vec<EnvelopePoint>)>, String> {
    let Some(percent) = &event.attack_dynamics else {
        return Ok(None);
    };
    let exact = percent * &"9/10".parse::<Rational>().unwrap();
    let value = exact.to_f64();
    if exact.is_negative()
        || exact > Rational::from_integer(127)
        || !value.is_finite()
        || (exact.is_positive() && value == 0.)
    {
        return Err(format!(
            "note_dynamics.range: {} has an unrepresentable NoteOn value",
            event.id
        ));
    }
    if !exact.is_positive() {
        return Ok(Some((0, vec![])));
    }
    let changing = envelope.windows(2).any(|p| p[0].gain != p[1].gain);
    if changing && policy.is_none() {
        return Err(format!(
            "note_dynamics.expression_unsupported: {} overlaps changing loudness; no sustain/trajectory policy is inferred",
            event.id
        ));
    }
    let velocity = value.round().clamp(1., 127.) as u8;
    let gain = value / f64::from(velocity);
    if changing {
        let first = envelope
            .first()
            .ok_or("note_dynamics.relative_baseline: missing envelope")?;
        if first.offset_seconds != 0. || !first.gain.is_finite() || first.gain <= 0. {
            return Err(format!(
                "note_dynamics.relative_baseline: {} requires finite nonzero onset expression",
                event.id
            ));
        }
        if envelope.iter().any(|p| {
            !p.offset_seconds.is_finite()
                || p.offset_seconds < 0.
                || p.offset_seconds > duration
                || !p.gain.is_finite()
                || p.gain < 0.
        }) || envelope
            .windows(2)
            .any(|p| p[0].offset_seconds > p[1].offset_seconds)
        {
            return Err("note_dynamics.relative_shape: invalid authored envelope".into());
        }
        let points = envelope
            .iter()
            .map(|p| {
                let normalized = gain * (p.gain / first.gain);
                if !normalized.is_finite() || (p.gain > 0. && normalized <= 0.) {
                    return Err(
                        "note_dynamics.relative_range: unrepresentable relative gain".to_string(),
                    );
                }
                Ok(EnvelopePoint {
                    offset_seconds: p.offset_seconds,
                    gain: normalized,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        return Ok(Some((velocity, points)));
    }

    Ok(Some((
        velocity,
        vec![
            EnvelopePoint {
                offset_seconds: 0.,
                gain,
            },
            EnvelopePoint {
                offset_seconds: duration,
                gain,
            },
        ],
    )))
}
