use scorestream_codec::{Rational, notation::ResolvedScore};
struct TempoPoint {
    at: Rational,
    seconds: Rational,
    seconds_per_quarter: Rational,
}
pub(super) struct TempoMap(Vec<TempoPoint>);
impl TempoMap {
    pub fn new(score: &ResolvedScore) -> Self {
        let mut points = vec![TempoPoint {
            at: Rational::zero(),
            seconds: Rational::zero(),
            seconds_per_quarter: "1/2".parse().unwrap(),
        }];
        for change in &score.tempos {
            let previous = points.last().unwrap();
            let seconds =
                &previous.seconds + &(&(&change.at - &previous.at) * &previous.seconds_per_quarter);
            points.push(TempoPoint {
                at: change.at.clone(),
                seconds,
                seconds_per_quarter: &Rational::from_integer(60)
                    / &(&change.value.beat.quarters() * &change.value.bpm),
            });
        }
        Self(points)
    }
    pub fn seconds(&self, at: &Rational) -> Result<f64, String> {
        let point = &self.0[self.0.partition_point(|p| p.at <= *at).saturating_sub(1)];
        let value = (&point.seconds + &(&(at - &point.at) * &point.seconds_per_quarter)).to_f64();
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
        let lo = self.0.partition_point(|p| p.at <= *start);
        let hi = self.0.partition_point(|p| p.at < *end);
        self.0[lo..hi].iter().map(|p| &p.at)
    }
}
