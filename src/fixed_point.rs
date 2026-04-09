use std::fmt;

/// A simple decimal fixed-point type for deterministic state representation on the Normal path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fixed(i128);

impl Fixed {
    pub const SCALE: i128 = 1_000_000_000;
    pub const ZERO: Self = Self(0);

    pub fn from_raw(raw: i128) -> Self {
        Self(raw)
    }

    pub fn raw(self) -> i128 {
        self.0
    }

    pub fn from_f64(value: f64) -> Result<Self, String> {
        if !value.is_finite() {
            return Err("fixed-point value must be finite".to_string());
        }

        let scaled = value * Self::SCALE as f64;
        if scaled.abs() > i128::MAX as f64 {
            return Err("fixed-point conversion overflow".to_string());
        }

        Ok(Self(scaled.round() as i128))
    }

    pub fn to_f64(self) -> f64 {
        self.0 as f64 / Self::SCALE as f64
    }

    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    pub fn max(self, other: Self) -> Self {
        if self >= other { self } else { other }
    }
}

impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.9}", self.to_f64())
    }
}
