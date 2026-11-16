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
    if envelope.windows(2).any(|p| p[0].gain != p[1].gain) {
        return Err(format!(
            "note_dynamics.expression_unsupported: {} overlaps changing loudness; no sustain/trajectory policy is inferred",
            event.id
        ));
    }
    let velocity = value.round().clamp(1., 127.) as u8;
    let gain = value / f64::from(velocity);
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
