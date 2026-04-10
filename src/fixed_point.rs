use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// A simple decimal fixed-point type for deterministic state representation on the Normal path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Fixed(i128);

impl Fixed {
    pub const SCALE: i128 = 1_000_000_000;
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(Self::SCALE);
    pub const TWO: Self = Self(Self::SCALE * 2);
    pub const LN_2: Self = Self(693_147_181);
    pub const SQRT_PI: Self = Self(1_772_453_851);
    pub const INV_SQRT_TWO_PI: Self = Self(398_942_280);

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

    pub fn min(self, other: Self) -> Self {
        if self <= other { self } else { other }
    }

    pub fn mul_int(self, rhs: i128) -> Self {
        Self(self.0 * rhs)
    }

    pub fn div_int(self, rhs: i128) -> Self {
        Self(self.0 / rhs)
    }

    pub fn sqrt(self) -> Result<Self, String> {
        if self.0 < 0 {
            return Err("cannot take square root of a negative fixed-point value".to_string());
        }
        if self.0 == 0 {
            return Ok(Self::ZERO);
        }

        let scaled = (self.0 as u128)
            .checked_mul(Self::SCALE as u128)
            .ok_or_else(|| "fixed-point sqrt overflow".to_string())?;
        Ok(Self(integer_sqrt(scaled) as i128))
    }

    pub fn exp_neg(self) -> Result<Self, String> {
        if self.0 < 0 {
            return Err("exp_neg expects a non-negative exponent".to_string());
        }

        let ln2_raw = Self::LN_2.raw();
        let shifts = self.0 / ln2_raw;
        let remainder = Self(self.0 % ln2_raw);
        let mut term = Self::ONE;
        let mut sum = Self::ONE;

        for n in 1..=18_i128 {
            term = -(term * remainder).div_int(n);
            sum = sum + term;
        }

        let shifted = if shifts >= 60 {
            Self::ZERO
        } else {
            Self(sum.raw() >> shifts)
        };

        Ok(shifted.max(Self::ZERO))
    }
}

impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.9}", self.to_f64())
    }
}

impl Add for Fixed {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Fixed {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul for Fixed {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self((self.0 * rhs.0) / Self::SCALE)
    }
}

impl Div for Fixed {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self((self.0 * Self::SCALE) / rhs.0)
    }
}

impl Neg for Fixed {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

fn integer_sqrt(value: u128) -> u128 {
    if value <= 1 {
        return value;
    }

    let mut x0 = value;
    let mut x1 = (x0 + value / x0) / 2;
    while x1 < x0 {
        x0 = x1;
        x1 = (x0 + value / x0) / 2;
    }
    x0
}
