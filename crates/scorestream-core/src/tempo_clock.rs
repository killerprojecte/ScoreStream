//! Exact piecewise tempo integration shared by explicit schedules and playback.
//! It changes no score positions and has no grace or fermata interpretation.
use crate::{notation::ResolvedScore, Rational};

#[derive(Clone, Debug, PartialEq, Eq)]
struct TempoPoint {
    at: Rational,
    seconds: Rational,
    seconds_per_quarter: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExactTempoMap {
    points: Vec<TempoPoint>,
    end: Rational,
    maximum_bits: Option<u64>,
}
impl ExactTempoMap {
    /// `maximum_bits` bounds every input and accumulated exact value. Ordinary
    /// legacy playback passes None; qualified constructors use a finite budget.
    pub fn new(score: &ResolvedScore, maximum_bits: Option<u64>) -> Result<Self, String> {
        let check = |r: &Rational| -> Result<(), String> {
            if maximum_bits.is_some_and(|n| r.numerator().bits() > n || r.denominator().bits() > n)
            {
                Err("Exact tempo value exceeds rational bit budget".into())
            } else {
                Ok(())
            }
        };
        check(&score.end)?;
        if score.end.is_negative() {
            return Err("Exact tempo score end must be nonnegative".into());
        }
        let mut points = vec![TempoPoint {
            at: Rational::zero(),
            seconds: Rational::zero(),
            seconds_per_quarter: "1/2".parse().unwrap(),
        }];
        for change in &score.tempos {
            check(&change.at)?;
            check(&change.value.bpm)?;
            let previous = points.last().unwrap();
            if change.at < previous.at || change.at > score.end || !change.value.bpm.is_positive() {
                return Err("Invalid exact tempo interval or rate".into());
            }
            let elapsed = &change.at - &previous.at;
            check(&elapsed)?;
            let elapsed_seconds = &elapsed * &previous.seconds_per_quarter;
            check(&elapsed_seconds)?;
            let seconds = &previous.seconds + &elapsed_seconds;
            check(&seconds)?;
            let rate = &change.value.beat.quarters() * &change.value.bpm;
            check(&rate)?;
            let seconds_per_quarter = &Rational::from_integer(60) / &rate;
            check(&seconds_per_quarter)?;
            let point = TempoPoint {
                at: change.at.clone(),
                seconds,
                seconds_per_quarter,
            };
            // Only the final same-time control is effective. Earlier same-time
            // writes do not create a nonconstant musical interval.
            if points.last().unwrap().at == point.at {
                *points.last_mut().unwrap() = point;
            } else {
                points.push(point);
            }
        }
        let out = Self {
            points,
            end: score.end.clone(),
            maximum_bits,
        };
        out.checked_seconds(&score.end)?;
        Ok(out)
    }
    pub fn quarter_end(&self) -> &Rational {
        &self.end
    }
    /// Exact integration, extrapolating outside the score domain for legacy
    /// callers. Source-qualified schedules enforce their own query range.
    pub fn seconds(&self, at: &Rational) -> Rational {
        let point = &self.points[self
            .points
            .partition_point(|p| &p.at <= at)
            .saturating_sub(1)];
        &point.seconds + &(&(at - &point.at) * &point.seconds_per_quarter)
    }
    /// Bounded exact conversion, including subtraction/multiplication before
    /// addition. The unbounded conversion remains available for legacy callers.
    pub fn checked_seconds(&self, at: &Rational) -> Result<Rational, String> {
        let check = |r: &Rational| {
            if self
                .maximum_bits
                .is_some_and(|n| r.numerator().bits() > n || r.denominator().bits() > n)
            {
                Err("Exact tempo value exceeds rational bit budget".to_owned())
            } else {
                Ok(())
            }
        };
        check(at)?;
        let point = &self.points[self
            .points
            .partition_point(|p| &p.at <= at)
            .saturating_sub(1)];
        let delta = at - &point.at;
        check(&delta)?;
        let elapsed = &delta * &point.seconds_per_quarter;
        check(&elapsed)?;
        let seconds = &point.seconds + &elapsed;
        check(&seconds)?;
        Ok(seconds)
    }
    /// Exact inverse, clamped to the original score domain.
    pub fn seek(&self, seconds: &Rational) -> Rational {
        let seconds = seconds
            .clone()
            .max(Rational::zero())
            .min(self.seconds(&self.end));
        let point = &self.points[self
            .points
            .partition_point(|p| p.seconds <= seconds)
            .saturating_sub(1)];
        &point.at + &(&(&seconds - &point.seconds) / &point.seconds_per_quarter)
    }
    pub fn boundaries<'a>(
        &'a self,
        start: &Rational,
        end: &Rational,
    ) -> impl Iterator<Item = &'a Rational> {
        let lo = self.points.partition_point(|p| &p.at <= start);
        let hi = self.points.partition_point(|p| &p.at < end);
        self.points[lo..hi.max(lo)].iter().map(|p| &p.at)
    }
    /// Redundant same-rate checkpoints do not make a percentage ambiguous.
    pub fn constant_between(&self, start: &Rational, end: &Rational) -> bool {
        if start.is_negative() || end < start || end > &self.end {
            return false;
        }
        let index = self
            .points
            .partition_point(|p| &p.at <= start)
            .saturating_sub(1);
        let initial = &self.points[index].seconds_per_quarter;
        self.points[index + 1..]
            .iter()
            .take_while(|p| &p.at < end)
            .all(|p| &p.seconds_per_quarter == initial)
    }
}
