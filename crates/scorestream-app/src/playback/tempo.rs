use scorestream_codec::{Rational, notation::ResolvedScore, tempo_clock::ExactTempoMap};
pub(super) struct TempoMap(ExactTempoMap);
impl TempoMap {
    pub fn new(score: &ResolvedScore) -> Result<Self, String> {
        ExactTempoMap::new(score, None).map(Self)
    }
    pub fn seconds(&self, at: &Rational) -> Result<f64, String> {
        let value = self.0.seconds(at).to_f64();
        if value.is_finite() && (0.0..=21_600.).contains(&value) {
            Ok(value)
        } else {
            Err("Playback timeline must fit in six hours at the specified tempo".into())
        }
    }
    pub fn boundaries<'a>(
        &'a self,
        start: &Rational,
        end: &Rational,
    ) -> impl Iterator<Item = &'a Rational> {
        self.0.boundaries(start, end)
    }
}
