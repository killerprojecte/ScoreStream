use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
    fmt,
    ops::{Add, Div, Mul, Sub},
    str::FromStr,
};

/// Arbitrary-precision, reduced exact musical value. JSON is always a string.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rational(BigRational);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RationalError {
    pub code: &'static str,
    pub message: String,
}
impl fmt::Display for RationalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for RationalError {}
impl Rational {
    pub fn zero() -> Self {
        Self(BigRational::zero())
    }
    pub fn one() -> Self {
        Self(BigRational::one())
    }
    pub fn from_integer(value: i64) -> Self {
        Self(BigRational::from_integer(value.into()))
    }
    pub fn from_big_integer(value: BigInt) -> Self {
        Self(BigRational::from_integer(value))
    }
    pub fn is_positive(&self) -> bool {
        self > &Self::zero()
    }
    pub fn is_negative(&self) -> bool {
        self < &Self::zero()
    }
    /// Approximation for display/projection only, never for semantic comparisons.
    pub fn to_f64(&self) -> f64 {
        self.0.to_f64().unwrap_or(if self.is_negative() {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        })
    }
    pub fn numerator(&self) -> &BigInt {
        self.0.numer()
    }
    pub fn denominator(&self) -> &BigInt {
        self.0.denom()
    }
    /// Accept only canonical integer/fraction spelling. A leading `+` is a pitch
    /// syntax concern and is deliberately not accepted in time/API values.
    pub fn parse_strict(text: &str) -> Result<Self, RationalError> {
        let err = |code, message: &str| RationalError {
            code,
            message: format!("{message}: {text}"),
        };
        let (n, d) = text.split_once('/').unwrap_or((text, "1"));
        let unsigned = n.strip_prefix('-').unwrap_or(n);
        if !digits(unsigned)
            || !digits(d)
            || unsigned.len() > 1 && unsigned.starts_with('0')
            || d.starts_with('0')
        {
            return Err(err(
                "rational_lexical",
                "Expected an integer or fraction with a positive denominator",
            ));
        }
        let numerator =
            BigInt::from_str(n).map_err(|_| err("rational_lexical", "Invalid numerator"))?;
        let denominator =
            BigInt::from_str(d).map_err(|_| err("rational_lexical", "Invalid denominator"))?;
        if denominator.is_zero() {
            return Err(err("rational_lexical", "Denominator cannot be zero"));
        }
        let value = Self(BigRational::new(numerator, denominator));
        if value.to_string() != text {
            return Err(err(
                "rational_not_canonical",
                "Use a reduced rational without negative zero",
            ));
        }
        Ok(value)
    }
}
fn digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}
impl Default for Rational {
    fn default() -> Self {
        Self::zero()
    }
}
impl FromStr for Rational {
    type Err = RationalError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_strict(s)
    }
}
impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.denom().is_one() {
            write!(f, "{}", self.0.numer())
        } else {
            write!(f, "{}/{}", self.0.numer(), self.0.denom())
        }
    }
}
impl Add for &Rational {
    type Output = Rational;
    fn add(self, other: Self) -> Rational {
        Rational(&self.0 + &other.0)
    }
}
impl Sub for &Rational {
    type Output = Rational;
    fn sub(self, other: Self) -> Rational {
        Rational(&self.0 - &other.0)
    }
}
impl Mul for &Rational {
    type Output = Rational;
    fn mul(self, other: Self) -> Rational {
        Rational(&self.0 * &other.0)
    }
}
impl Serialize for Rational {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for Rational {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

impl Div for &Rational {
    type Output = Rational;
    fn div(self, other: Self) -> Rational {
        Rational(&self.0 / &other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arbitrary_precision_is_exact() {
        let n: Rational = "33333333333333333333333333333333333333333333333"
            .parse()
            .unwrap();
        assert_eq!(
            (&n + &n).to_string(),
            "66666666666666666666666666666666666666666666666"
        );
        let third: Rational = "1/3".parse().unwrap();
        assert_eq!((&(&third + &third) + &third).to_string(), "1");
    }
    #[test]
    fn strict_and_string_only() {
        for bad in ["2/4", "0/3", "-0", "1/0", "1/-2", "0.5", "+1", "01"] {
            assert!(bad.parse::<Rational>().is_err(), "{bad}");
        }
        assert!(serde_json::from_str::<Rational>("0.5").is_err());
        assert_eq!(
            serde_json::to_string(&"1/3".parse::<Rational>().unwrap()).unwrap(),
            "\"1/3\""
        );
    }
}
