//! Stable path/source-free IPC error classification shared by preflight and replay.
pub(super) fn audio_code(error: &(dyn std::error::Error + 'static), fallback: &'static str) -> &'static str {
    if matches!(error.downcast_ref::<scorestream_audio::AudioError>(), Some(scorestream_audio::AudioError::Cancelled)) {
        "OFFLINE_CANCELLED"
    } else if matches!(error.downcast_ref::<scorestream_audio::AudioError>(), Some(scorestream_audio::AudioError::Unavailable(_))) {
        "OFFLINE_BACKEND_UNAVAILABLE"
    } else { fallback }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_cancellation_is_identical_before_and_during_replay() {
        for fallback in ["OFFLINE_PREFLIGHT_FAILED", "OFFLINE_RENDER_FAILED"] {
            assert_eq!(audio_code(&scorestream_audio::AudioError::Cancelled, fallback), "OFFLINE_CANCELLED");
            assert_eq!(audio_code(&scorestream_audio::AudioError::Native("private path".into()), fallback), fallback);
            assert_eq!(audio_code(&scorestream_audio::AudioError::Unavailable("private path".into()), fallback), "OFFLINE_BACKEND_UNAVAILABLE");
            // Mere text is not a typed native cancellation receipt.
            assert_eq!(audio_code(&std::io::Error::other("cancelled"), fallback), fallback);
        }
    }
}
