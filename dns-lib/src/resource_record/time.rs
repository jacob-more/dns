use std::{
    error::Error,
    fmt::Display,
    iter::Sum,
    num::ParseIntError,
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign},
    time::Duration,
};

use dns_macros::{FromWire, ToPresentation, ToWire};

use crate::serde::presentation::{errors::TokenError, from_presentation::FromPresentation};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TimeError {
    DateTimeError(DateTimeError),
    InvalidTime,
}
impl Error for TimeError {}
impl Display for TimeError {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeError::DateTimeError(error) => write!(f, "{error}"),
            TimeError::InvalidTime => write! {f, "invalid time"},
        }
    }
}
impl From<DateTimeError> for TimeError {
    #[inline]
    fn from(value: DateTimeError) -> Self {
        Self::DateTimeError(value)
    }
}

/// All ttl's are of the type u32, as per https://datatracker.ietf.org/doc/html/rfc2181#section-8
type TimeInt = u32;

/// https://datatracker.ietf.org/doc/html/rfc2181#section-8
pub const TTL_MAX: TimeInt = 2_u32.pow(31) - 1;
/// https://datatracker.ietf.org/doc/html/rfc2181#section-8
pub const TTL_MIN: TimeInt = 0;

#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Debug,
    ToWire,
    FromWire,
    ToPresentation,
)]
pub struct Time {
    ttl: TimeInt,
}

impl Time {
    pub const ZERO: Self = Self::from_secs(0);
    pub const ONE: Self = Self::from_secs(1);
    pub const TWO: Self = Self::from_secs(2);
    pub const THREE: Self = Self::from_secs(3);
    pub const FOUR: Self = Self::from_secs(4);
    pub const FIVE: Self = Self::from_secs(5);
    pub const SIX: Self = Self::from_secs(6);
    pub const SEVEN: Self = Self::from_secs(7);
    pub const EIGHT: Self = Self::from_secs(8);
    pub const NINE: Self = Self::from_secs(9);
    pub const TEN: Self = Self::from_secs(10);

    pub const MAX: Self = Self::from_secs(TTL_MAX);
    pub const MIN: Self = Self::from_secs(TTL_MIN);

    /// Creates a new `TTL` from the specified number of whole seconds.
    #[inline]
    pub const fn new(seconds: TimeInt) -> Self {
        Self { ttl: seconds }
    }

    /// Creates a new `TTL` from the specified number of whole seconds.
    #[inline]
    pub const fn from_secs(seconds: TimeInt) -> Self {
        Self::new(seconds)
    }

    /// Creates a new `Some(TTL)` from the specified [`Duration`] if it is
    /// within the range [`TTL::MIN`] - [`TTL:MAX`] or [`None`] otherwise.
    #[inline]
    pub const fn checked_from_duration(duration: Duration) -> Option<Self> {
        let second = duration.as_secs();
        // Note: we don't actually need to check the lower bound because the
        //       lower bound of Duration is also zero.
        if duration.as_secs() > TTL_MAX as u64 {
            None
        } else {
            Some(Self {
                ttl: second as TimeInt,
            })
        }
    }

    /// Creates a new `Some(TTL)` from the specified [`Duration`] if it is
    /// within the range [`TTL::MIN`] - [`TTL:MAX`] returning [`TTL::ZERO`]
    /// if the result would be negative or [`TTL::MAX`] if overflow occurred.
    #[inline]
    pub const fn saturating_from_duration(duration: Duration) -> Self {
        match Self::checked_from_duration(duration) {
            Some(ttl) => ttl,
            None => Self::MAX,
        }
    }

    /// Returns true if this `TTL` spans no time.
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.ttl == 0
    }

    /// Returns the number of _whole_ seconds contained by this `TTL`.
    #[inline]
    pub const fn as_secs(&self) -> TimeInt {
        self.ttl
    }

    /// Returns the `Duration` contained by this `TTL`.
    #[inline]
    pub const fn as_duration(&self) -> Duration {
        Duration::from_secs(self.ttl as u64)
    }

    /// Checked `TTL` addition. Computes `self + other`, returning [`None`]
    /// if overflow occurred.
    #[inline]
    pub const fn checked_add(self, rhs: Self) -> Option<Self> {
        match self.ttl.checked_add(rhs.ttl) {
            // TTL is a subset of Duration. Check upper bound again.
            Some(seconds) => {
                // Note: no need to check lower bound (zero) because durations
                //       have the same lower bound.
                if seconds > TTL_MAX {
                    None
                } else {
                    Some(Self { ttl: seconds })
                }
            }
            None => None,
        }
    }

    /// Saturating [`TTL`] addition. Computes `self + other`, returning [`TTL::MAX`]
    /// if overflow occurred.
    #[inline]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        match self.checked_add(rhs) {
            Some(res) => res,
            None => Self::MAX,
        }
    }

    /// Checked `TTL` subtraction. Computes `self - other`, returning [`None`]
    /// if the result would be negative or if overflow occurred.
    #[inline]
    pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
        match self.ttl.checked_sub(rhs.ttl) {
            // Note: no need to check lower bound (zero) because durations
            //       have the same lower bound.
            Some(seconds) => Some(Self { ttl: seconds }),
            None => None,
        }
    }

    /// Saturating `TTL` subtraction. Computes `self - other`, returning [`TTL::ZERO`]
    /// if the result would be negative or if overflow occurred.
    #[inline]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        match self.checked_sub(rhs) {
            Some(res) => res,
            None => Self::ZERO,
        }
    }

    /// Checked `TTL` multiplication. Computes `self * other`, returning
    /// [`None`] if overflow occurred.
    #[inline]
    pub const fn checked_mul(self, rhs: TimeInt) -> Option<Self> {
        match self.ttl.checked_mul(rhs) {
            // TTL is a subset of Duration. Check upper bound again.
            Some(seconds) => {
                // Note: no need to check lower bound (zero) because durations
                //       have the same lower bound.
                if seconds > TTL_MAX {
                    None
                } else {
                    Some(Self { ttl: seconds })
                }
            }
            None => None,
        }
    }

    /// Saturating `TTL` multiplication. Computes `self * other`, returning
    /// [`TTL::MAX`] if overflow occurred.
    #[inline]
    pub const fn saturating_mul(self, rhs: TimeInt) -> Self {
        match self.checked_mul(rhs) {
            Some(res) => res,
            None => Self::MAX,
        }
    }

    /// Checked `TTL` division. Computes `self / other`, returning [`None`]
    /// if `other == 0`.
    #[inline]
    pub const fn checked_div(self, rhs: TimeInt) -> Option<Self> {
        match self.ttl.checked_div(rhs) {
            // Note: no need to check lower bound (zero) because durations
            //       have the same lower bound.
            Some(seconds) => Some(Self { ttl: seconds }),
            None => None,
        }
    }

    /// Saturating `TTL` division. Computes `self / other`, returning
    /// [`TTL::ZERO`] if the result would be negative or if overflow occurred.
    #[inline]
    pub const fn saturating_div(self, rhs: TimeInt) -> Self {
        match self.checked_mul(rhs) {
            Some(res) => res,
            None => Self::ZERO,
        }
    }
}

impl Add for Time {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        self.checked_add(rhs)
            .expect("overflow when adding durations")
    }
}

impl AddAssign for Time {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Time {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        self.checked_sub(rhs)
            .expect("overflow when subtracting durations")
    }
}

impl SubAssign for Time {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Mul<TimeInt> for Time {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: TimeInt) -> Self {
        self.checked_mul(rhs)
            .expect("overflow when multiplying duration by scalar")
    }
}

impl Mul<Time> for TimeInt {
    type Output = Time;

    #[inline]
    fn mul(self, rhs: Time) -> Time {
        rhs * self
    }
}

impl MulAssign<TimeInt> for Time {
    #[inline]
    fn mul_assign(&mut self, rhs: TimeInt) {
        *self = *self * rhs;
    }
}

impl Div<TimeInt> for Time {
    type Output = Self;

    #[inline]
    fn div(self, rhs: TimeInt) -> Self {
        self.checked_div(rhs)
            .expect("divide by zero error when dividing duration by scalar")
    }
}

impl DivAssign<TimeInt> for Time {
    #[inline]
    fn div_assign(&mut self, rhs: TimeInt) {
        *self = *self / rhs;
    }
}

macro_rules! sum_durations {
    ($iter:expr) => {{
        let mut total_secs: TimeInt = 0;

        for entry in $iter {
            total_secs = total_secs
                .checked_add(entry.ttl)
                .expect("overflow in iter::sum over durations");
        }
        Time::from_secs(total_secs)
    }};
}

impl Sum for Time {
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        sum_durations!(iter)
    }
}

impl<'a> Sum<&'a Self> for Time {
    #[inline]
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        sum_durations!(iter)
    }
}

// Updated TTL Parsing: https://datatracker.ietf.org/doc/html/rfc4034#section-3.2

/// Maximum number of digits that can occur in a u32 integer.
const U32_MAX_DIGITS: usize = 10;
const DATE_TIME_DIGITS: usize = 14;

impl FromPresentation for Time {
    #[inline]
    fn from_token_format<'a, 'b, 'c, 'd>(
        tokens: &'c [&'a str],
    ) -> Result<(Self, &'d [&'a str]), TokenError>
    where
        Self: Sized,
        'a: 'b,
        'c: 'd,
        'c: 'd,
    {
        match tokens {
            [] => Err(TokenError::OutOfTokens),
            [token, ..] => {
                let (seconds, tokens) = match token.len() {
                    ..=U32_MAX_DIGITS => TimeInt::from_token_format(tokens)?,
                    DATE_TIME_DIGITS => (datetime_parse(token)?, &tokens[1..]),
                    _ => return Err(TimeError::InvalidTime)?,
                };
                if seconds <= TTL_MAX {
                    Ok((Self::new(seconds), tokens))
                } else {
                    Err(TimeError::InvalidTime)?
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DateTimeError {
    IncorrectNumberOfDigits(TimeInt),
    IntegerParseError(ParseIntError),
    YearTooLarge(TimeInt),
    YearTooSmall(TimeInt),
    MonthTooLarge(TimeInt),
    MonthTooSmall(TimeInt),
    DayTooLarge(TimeInt),
    DayTooSmall(TimeInt),
    HourTooLarge(TimeInt),
    HourTooSmall(TimeInt),
    MinuteTooLarge(TimeInt),
    MinuteTooSmall(TimeInt),
    SecondTooLarge(TimeInt),
    SecondTooSmall(TimeInt),
    IntegerOverflow,
}
impl Error for DateTimeError {}
impl Display for DateTimeError {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IncorrectNumberOfDigits(value) => write!(
                f,
                "IncorrectNumberOfDigits: expected 14 digits, received {value}"
            ),
            Self::IntegerParseError(error) => write!(f, "IntegerParseError: {error}"),
            Self::YearTooLarge(value) => write!(f, "YearTooLarge: integer received was {value}"),
            Self::YearTooSmall(value) => write!(f, "YearTooSmall: integer received was {value}"),
            Self::MonthTooLarge(value) => write!(f, "MonthTooLarge: integer received was {value}"),
            Self::MonthTooSmall(value) => write!(f, "MonthTooSmall: integer received was {value}"),
            Self::DayTooLarge(value) => write!(f, "DayTooLarge: integer received was {value}"),
            Self::DayTooSmall(value) => write!(f, "DayTooSmall: integer received was {value}"),
            Self::HourTooLarge(value) => write!(f, "HourTooLarge: integer received was {value}"),
            Self::HourTooSmall(value) => write!(f, "HourTooSmall: integer received was {value}"),
            Self::MinuteTooLarge(value) => {
                write!(f, "MinuteTooLarge: integer received was {value}")
            }
            Self::MinuteTooSmall(value) => {
                write!(f, "MinuteTooSmall: integer received was {value}")
            }
            Self::SecondTooLarge(value) => {
                write!(f, "SecondTooLarge: integer received was {value}")
            }
            Self::SecondTooSmall(value) => {
                write!(f, "SecondTooSmall: integer received was {value}")
            }
            Self::IntegerOverflow => write!(
                f,
                "IntegerOverflow: integer overflowed while finding the number of seconds since epoch"
            ),
        }
    }
}
impl From<ParseIntError> for DateTimeError {
    #[inline]
    fn from(value: ParseIntError) -> Self {
        Self::IntegerParseError(value)
    }
}

#[inline]
fn minutes_to_seconds(minutes: TimeInt) -> Option<TimeInt> {
    minutes.checked_mul(60)
}

#[inline]
fn hours_to_seconds(hours: TimeInt) -> Option<TimeInt> {
    minutes_to_seconds(hours.checked_mul(60)?)
}

#[inline]
fn days_to_seconds(days: TimeInt) -> Option<TimeInt> {
    hours_to_seconds(days.checked_mul(24)?)
}

#[inline]
fn month_to_days(month: TimeInt, year: TimeInt) -> TimeInt {
    match (month, year % 4) {
        // Months with 31 days. Leap year does not matter.
        (1, _) | (3, _) | (5, _) | (7, _) | (8, _) | (10, _) | (12, _) => 31,
        // Months with 30 days. Leap year does not matter.
        (4, _) | (6, _) | (9, _) | (11, _) => 30,
        // February with 29. Leap year only.
        (2, 0) => 29,
        // February with 28. Non-leap year only.
        (2, _) => 28,
        _ => panic!("Invalid month in the year. Acceptable inputs are 1-12"),
    }
}

#[inline]
fn months_to_seconds(months: TimeInt, year: TimeInt) -> Option<TimeInt> {
    let days = (1..=months).map(|month| month_to_days(month, year)).sum();
    days_to_seconds(days)
}

// #[inline]
// fn year_to_seconds(year: TimeInt) -> Option<TimeInt> {
//     match year % 4 {
//         // Leap year
//         0 => days_to_seconds(year.checked_mul(366)?),
//         // Non-leap year
//         _ => days_to_seconds(year.checked_mul(365)?),
//     }
// }

#[inline]
fn years_since_1970_to_seconds(years: TimeInt) -> Option<TimeInt> {
    let leap_years = years / 4;
    let non_leap_years = years - leap_years;
    days_to_seconds(leap_years.checked_mul(366)?)?.checked_add(non_leap_years.checked_mul(365)?)
}

#[inline]
fn seconds_since_1970(
    year: TimeInt,
    month: TimeInt,
    day: TimeInt,
    hour: TimeInt,
    minute: TimeInt,
    second: TimeInt,
) -> Option<TimeInt> {
    let mut total_second = 0_u32;
    if year > 1970 {
        total_second = total_second.checked_add(years_since_1970_to_seconds(1970 - year - 1)?)?;
    }
    if month > 1 {
        total_second = total_second.checked_add(months_to_seconds(month - 1, year)?)?;
    }
    if day > 1 {
        total_second = total_second.checked_add(days_to_seconds(day - 1)?)?;
    }
    if hour > 0 {
        total_second = total_second.checked_add(hours_to_seconds(hour - 1)?)?;
    }
    if minute > 1 {
        total_second = total_second.checked_add(minutes_to_seconds(minute - 1)?)?;
    }
    total_second.checked_add(second)
}

#[inline]
fn datetime_parse<'a, 'b>(token: &'a str) -> Result<TimeInt, DateTimeError>
where
    'a: 'b,
{
    if token.len() < DATE_TIME_DIGITS {
        todo!("Error: cannot parse");
    }

    let year = match TimeInt::from_str_radix(&token[0..4], 10)? {
        year @ 0 => return Err(DateTimeError::YearTooLarge(year)),
        year @ 1..=9999 => year,
        year @ 1000.. => return Err(DateTimeError::YearTooSmall(year)),
    };
    let month = match TimeInt::from_str_radix(&token[4..6], 10)? {
        month @ 0 => return Err(DateTimeError::MonthTooLarge(month)),
        month @ 1..=12 => month,
        month @ 13.. => return Err(DateTimeError::MonthTooSmall(month)),
    };
    let day = match TimeInt::from_str_radix(&token[6..8], 10)? {
        day @ 0 => return Err(DateTimeError::DayTooLarge(day)),
        day @ 1..=31 => day,
        day @ 32.. => return Err(DateTimeError::DayTooSmall(day)),
    };
    let hour = match TimeInt::from_str_radix(&token[8..10], 10)? {
        hour @ 0..=23 => hour,
        hour @ 24.. => return Err(DateTimeError::HourTooLarge(hour)),
    };
    let minute = match TimeInt::from_str_radix(&token[10..12], 10)? {
        minute @ 0..=59 => minute,
        minute @ 60.. => return Err(DateTimeError::MinuteTooLarge(minute)),
    };
    let second = match TimeInt::from_str_radix(&token[12..14], 10)? {
        second @ 0..=59 => second,
        second @ 60.. => return Err(DateTimeError::SecondTooLarge(second)),
    };

    match seconds_since_1970(year, month, day, hour, minute, second) {
        Some(seconds) => Ok(seconds),
        None => Err(DateTimeError::IntegerOverflow),
    }
}

#[cfg(test)]
mod circular_serde_sanity_test {
    use super::{TTL_MAX, TTL_MIN, Time};
    use crate::serde::wire::circular_test::gen_test_circular_serde_sanity_test;

    gen_test_circular_serde_sanity_test!(
        min_record_circular_serde_sanity_test,
        Time { ttl: TTL_MIN }
    );
    gen_test_circular_serde_sanity_test!(
        max_record_circular_serde_sanity_test,
        Time { ttl: TTL_MAX }
    );
    gen_test_circular_serde_sanity_test!(one_record_circular_serde_sanity_test, Time { ttl: 1 });
    gen_test_circular_serde_sanity_test!(record_circular_serde_sanity_test, Time { ttl: 86400 });
}

#[cfg(test)]
mod tokenizer_tests {
    use super::Time;
    use crate::{
        resource_record::time::{TTL_MAX, TTL_MIN},
        serde::presentation::test_from_presentation::{gen_fail_token_test, gen_ok_token_test},
    };

    gen_fail_token_test!(test_fail_u32_illegal_chars, Time, &["characters"]);
    gen_fail_token_test!(test_fail_u32_empty_str, Time, &[""]);

    // u32 tests
    gen_ok_token_test!(test_ok_u32_min, Time, Time { ttl: TTL_MIN }, &["0"]);
    gen_ok_token_test!(
        test_ok_u32_max,
        Time,
        Time { ttl: TTL_MAX },
        &["2147483647"]
    );

    // datetime tests
    gen_ok_token_test!(
        test_ok_date_time_min,
        Time,
        Time { ttl: 0 },
        &["00010101000000"]
    );
    gen_ok_token_test!(
        test_ok_date_time_one,
        Time,
        Time { ttl: 1 },
        &["00010101000001"]
    );
    gen_fail_token_test!(
        test_fail_date_time_seconds_overflow,
        Time,
        &["00010101000060"]
    );
    gen_fail_token_test!(
        test_fail_date_time_minutes_overflow,
        Time,
        &["00010101006000"]
    );
    gen_fail_token_test!(
        test_fail_date_time_hours_overflow,
        Time,
        &["00010101240000"]
    );
    gen_fail_token_test!(test_fail_date_time_days_overflow, Time, &["00010132000000"]);
    gen_fail_token_test!(
        test_fail_date_time_month_overflow,
        Time,
        &["00011301000000"]
    );
    gen_fail_token_test!(
        test_fail_date_time_digit_overflow,
        Time,
        &["100010101000000"]
    );
    gen_fail_token_test!(
        test_fail_date_time_digit_underflow,
        Time,
        &["0010101000000"]
    );
}
