//! Dates in the Gregorian calendar, and today's date (M25, D0030).
//!
//! The sums are Dovetail's own: the days of each month, the day of the
//! week, and adding days and months. Days are counted from 1970-01-01,
//! with Howard Hinnant's algorithms, for every year from -9999 to 9999.
//!
//! [`today`] is the local date: one a test set with [`set_today`], else
//! the system's local date from the source the desktop crate installs
//! with [`set_date_source`], else, with neither, the date in UTC.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

/// The earliest year a [`Date`] can have.
pub const MIN_YEAR: i32 = -9999;
/// The latest year a [`Date`] can have.
pub const MAX_YEAR: i32 = 9999;

/// A day in the Gregorian calendar, with no time or time zone.
///
/// Dates compare in time order. Text is year-month-day, as `2026-11-27`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
  year: i32,
  month: u8,
  day: u8,
}

/// A day of the week.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Weekday {
  /// Monday.
  Monday,
  /// Tuesday.
  Tuesday,
  /// Wednesday.
  Wednesday,
  /// Thursday.
  Thursday,
  /// Friday.
  Friday,
  /// Saturday.
  Saturday,
  /// Sunday.
  Sunday,
}

impl Weekday {
  /// Every day, from Monday.
  pub const ALL: [Weekday; 7] = [
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
    Weekday::Saturday,
    Weekday::Sunday,
  ];

  /// Its place from Monday, 0 to 6.
  pub fn from_monday(self) -> u32 {
    self as u32
  }

  /// Its name in English (M22 brings other languages).
  pub fn name(self) -> &'static str {
    [
      "Monday",
      "Tuesday",
      "Wednesday",
      "Thursday",
      "Friday",
      "Saturday",
      "Sunday",
    ][self as usize]
  }

  /// The first two letters of its name.
  pub fn short(self) -> &'static str {
    &self.name()[..2]
  }
}

/// The names of the months in English, from January.
pub const MONTH_NAMES: [&str; 12] = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

/// True in a leap year of the Gregorian calendar.
pub const fn is_leap_year(year: i32) -> bool {
  (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// How many days `month` (1 to 12) of `year` has, or 0 for no month.
pub const fn days_in_month(
  year: i32,
  month: u32,
) -> u32 {
  match month {
    1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
    4 | 6 | 9 | 11 => 30,
    2 if is_leap_year(year) => 29,
    2 => 28,
    _ => 0,
  }
}

impl Date {
  /// The day `day` of `month` (1 to 12) in `year`, or `None` when there is
  /// no such day.
  pub const fn new(
    year: i32,
    month: u32,
    day: u32,
  ) -> Option<Date> {
    if year < MIN_YEAR
      || year > MAX_YEAR
      || day == 0
      || day > days_in_month(year, month)
    {
      return None;
    }
    Some(Date { year, month: month as u8, day: day as u8 })
  }

  /// The year.
  pub fn year(self) -> i32 {
    self.year
  }

  /// The month, from 1 (January) to 12.
  pub fn month(self) -> u32 {
    u32::from(self.month)
  }

  /// The day of the month, from 1.
  pub fn day(self) -> u32 {
    u32::from(self.day)
  }

  /// Days since 1970-01-01, negative before it.
  pub fn days_since_epoch(self) -> i64 {
    let y = i64::from(self.year) - i64::from(self.month <= 2);
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let m = i64::from(self.month);
    let day_of_year = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5
      + i64::from(self.day)
      - 1;
    let day_of_era =
      year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
  }

  /// The date `days` after 1970-01-01, or `None` outside the years a date
  /// can have.
  pub fn from_days_since_epoch(days: i64) -> Option<Date> {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524
      - day_of_era / 146_096)
      / 365;
    let day_of_year =
      day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    let year = i32::try_from(year).ok()?;
    Date::new(year, month as u32, day as u32)
  }

  /// The day of the week.
  pub fn weekday(self) -> Weekday {
    // 1970-01-01 was a Thursday.
    let from_monday = (self.days_since_epoch() + 3).rem_euclid(7);
    Weekday::ALL[from_monday as usize]
  }

  /// The date `days` later, or earlier when negative, or `None` outside
  /// the years a date can have.
  pub fn add_days(
    self,
    days: i64,
  ) -> Option<Date> {
    Date::from_days_since_epoch(self.days_since_epoch().checked_add(days)?)
  }

  /// The same day `months` later, or earlier when negative. A day past
  /// the end of the new month becomes its last day: a month after
  /// January 31 is the last day of February.
  pub fn add_months(
    self,
    months: i32,
  ) -> Option<Date> {
    let index =
      i64::from(self.year) * 12 + i64::from(self.month) - 1 + i64::from(months);
    let year = i32::try_from(index.div_euclid(12)).ok()?;
    let month = index.rem_euclid(12) as u32 + 1;
    let day = self.day().min(days_in_month(year, month));
    Date::new(year, month, day)
  }

  /// The first day of this date's month.
  pub fn first_of_month(self) -> Date {
    Date { day: 1, ..self }
  }

  /// How many days this date's month has.
  pub fn days_in_month(self) -> u32 {
    days_in_month(self.year, self.month())
  }

  /// Reads a date written year-month-day, such as `2026-11-27`, with or
  /// without leading zeros. Spaces around it are ignored.
  pub fn parse(text: &str) -> Option<Date> {
    let text = text.trim();
    let (negative, text) = match text.strip_prefix('-') {
      Some(rest) => (true, rest),
      None => (false, text),
    };
    let mut parts = text.split('-');
    let number = |part: Option<&str>| -> Option<u32> {
      let part = part?.trim();
      if part.is_empty()
        || part.len() > 4
        || !part.bytes().all(|b| b.is_ascii_digit())
      {
        return None;
      }
      part.parse().ok()
    };
    let year = number(parts.next())? as i32;
    let month = number(parts.next())?;
    let day = number(parts.next())?;
    if parts.next().is_some() {
      return None;
    }
    Date::new(if negative { -year } else { year }, month, day)
  }
}

impl fmt::Display for Date {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    if self.year < 0 {
      write!(f, "-{:04}-{:02}-{:02}", -self.year, self.month, self.day)
    } else {
      write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
  }
}

/// A time of day, to the second, with no date or time zone (M25).
///
/// Times compare in order through the day. Text is on the 24-hour clock,
/// as `14:05` or `14:05:09`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time {
  hour: u8,
  minute: u8,
  second: u8,
}

impl Time {
  /// Midnight.
  pub const MIDNIGHT: Time = Time { hour: 0, minute: 0, second: 0 };

  /// `hour` (0 to 23), `minute` and `second` (0 to 59), or `None` when
  /// one is out of range.
  pub const fn new(
    hour: u32,
    minute: u32,
    second: u32,
  ) -> Option<Time> {
    if hour > 23 || minute > 59 || second > 59 {
      return None;
    }
    Some(Time {
      hour: hour as u8,
      minute: minute as u8,
      second: second as u8,
    })
  }

  /// The hour, 0 to 23.
  pub fn hour(self) -> u32 {
    u32::from(self.hour)
  }

  /// The minute, 0 to 59.
  pub fn minute(self) -> u32 {
    u32::from(self.minute)
  }

  /// The second, 0 to 59.
  pub fn second(self) -> u32 {
    u32::from(self.second)
  }

  /// The hour on the 12-hour clock, 1 to 12, and whether it is after
  /// noon.
  pub fn hour12(self) -> (u32, bool) {
    let h = self.hour();
    (if h.is_multiple_of(12) { 12 } else { h % 12 }, h >= 12)
  }

  /// Seconds since midnight.
  pub fn seconds_since_midnight(self) -> u32 {
    self.hour() * 3600 + self.minute() * 60 + self.second()
  }

  /// Reads a time on the 24-hour clock, as `14:05` or `14:05:09`, or on
  /// the 12-hour clock with `am` or `pm` after it, as `2:05 pm`.
  pub fn parse(text: &str) -> Option<Time> {
    let lower = text.trim().to_ascii_lowercase();
    let (clock, pm) = if let Some(rest) = lower.strip_suffix("pm") {
      (rest.trim_end(), Some(true))
    } else if let Some(rest) = lower.strip_suffix("am") {
      (rest.trim_end(), Some(false))
    } else {
      (lower.as_str(), None)
    };
    let parts: Vec<&str> = clock.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
      return None;
    }
    let number = |part: &str| -> Option<u32> {
      if part.is_empty()
        || part.len() > 2
        || !part.bytes().all(|b| b.is_ascii_digit())
      {
        return None;
      }
      part.parse().ok()
    };
    let mut hour = number(parts[0])?;
    let minute = number(parts[1])?;
    let second = match parts.get(2) {
      Some(s) => number(s)?,
      None => 0,
    };
    if let Some(pm) = pm {
      if hour == 0 || hour > 12 {
        return None;
      }
      hour = hour % 12 + if pm { 12 } else { 0 };
    }
    Time::new(hour, minute, second)
  }
}

impl fmt::Display for Time {
  fn fmt(
    &self,
    f: &mut fmt::Formatter<'_>,
  ) -> fmt::Result {
    write!(f, "{:02}:{:02}", self.hour, self.minute)?;
    if self.second != 0 {
      write!(f, ":{:02}", self.second)?;
    }
    Ok(())
  }
}

/// Where [`today`] reads the system's local date.
pub type DateSource = Rc<dyn Fn() -> Option<Date>>;

thread_local! {
  static TODAY: Cell<Option<Date>> = const { Cell::new(None) };
  static SOURCE: RefCell<Option<DateSource>> = const { RefCell::new(None) };
}

/// Today's local date: the one set with [`set_today`], else the installed
/// source's, else the date in UTC.
pub fn today() -> Date {
  if let Some(date) = TODAY.get() {
    return date;
  }
  let source = SOURCE.with_borrow(Clone::clone);
  if let Some(date) = source.and_then(|s| s()) {
    return date;
  }
  utc_today()
}

/// Makes [`today`] give `date` on this thread, as tests do, or goes back
/// to the system's date with `None`.
pub fn set_today(date: Option<Date>) {
  TODAY.set(date);
}

/// Installs where [`today`] reads the system's local date on this thread,
/// or removes it with `None`. The desktop crate installs one.
pub fn set_date_source(source: Option<DateSource>) {
  SOURCE.with_borrow_mut(|s| *s = source);
}

/// Today's date in UTC, from the system clock.
fn utc_today() -> Date {
  let seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
    Ok(d) => d.as_secs() as i64,
    Err(e) => -(e.duration().as_secs() as i64),
  };
  Date::from_days_since_epoch(seconds.div_euclid(86_400)).unwrap_or(Date {
    year: 1970,
    month: 1,
    day: 1,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  fn d(
    y: i32,
    m: u32,
    day: u32,
  ) -> Date {
    Date::new(y, m, day).unwrap()
  }

  #[test]
  fn days_count_from_1970_across_leap_years() {
    assert_eq!(d(1970, 1, 1).days_since_epoch(), 0);
    assert_eq!(d(1969, 12, 31).days_since_epoch(), -1);
    assert_eq!(d(2000, 3, 1).days_since_epoch(), 11_017);
    assert_eq!(d(2026, 11, 27).days_since_epoch(), 20_784);
    assert_eq!(d(1600, 1, 1).days_since_epoch(), -135_140);
    for days in (-800_000..800_000).step_by(997) {
      let date = Date::from_days_since_epoch(days).unwrap();
      assert_eq!(date.days_since_epoch(), days, "{date}");
    }
  }

  #[test]
  fn months_have_the_right_lengths() {
    assert_eq!(days_in_month(2024, 2), 29);
    assert_eq!(days_in_month(2023, 2), 28);
    assert_eq!(days_in_month(1900, 2), 28, "a century is not a leap year");
    assert_eq!(days_in_month(2000, 2), 29, "unless it divides by 400");
    assert_eq!(days_in_month(2026, 4), 30);
    assert_eq!(days_in_month(2026, 13), 0);
    assert!(Date::new(2023, 2, 29).is_none());
    assert!(Date::new(2026, 0, 1).is_none());
    assert!(Date::new(2026, 1, 0).is_none());
  }

  #[test]
  fn weekdays_match_known_dates() {
    assert_eq!(d(1970, 1, 1).weekday(), Weekday::Thursday);
    assert_eq!(d(2000, 1, 1).weekday(), Weekday::Saturday);
    assert_eq!(d(2026, 11, 27).weekday(), Weekday::Friday);
    assert_eq!(d(2024, 2, 29).weekday(), Weekday::Thursday);
    assert_eq!(d(1066, 10, 14).weekday(), Weekday::Sunday, "proleptic");
  }

  #[test]
  fn adding_days_and_months_crosses_month_and_year_ends() {
    assert_eq!(d(2026, 12, 31).add_days(1), Some(d(2027, 1, 1)));
    assert_eq!(d(2024, 2, 28).add_days(1), Some(d(2024, 2, 29)));
    assert_eq!(d(2024, 3, 1).add_days(-1), Some(d(2024, 2, 29)));
    assert_eq!(d(2026, 1, 31).add_months(1), Some(d(2026, 2, 28)));
    assert_eq!(d(2024, 1, 31).add_months(1), Some(d(2024, 2, 29)));
    assert_eq!(d(2026, 1, 15).add_months(-1), Some(d(2025, 12, 15)));
    assert_eq!(d(2026, 3, 31).add_months(-13), Some(d(2025, 2, 28)));
    assert_eq!(d(9999, 12, 31).add_days(1), None);
    assert_eq!(d(2026, 11, 27).first_of_month(), d(2026, 11, 1));
  }

  #[test]
  fn dates_read_and_write_year_month_day() {
    assert_eq!(Date::parse("2026-11-27"), Some(d(2026, 11, 27)));
    assert_eq!(Date::parse(" 2026-1-5 "), Some(d(2026, 1, 5)));
    assert_eq!(d(2026, 1, 5).to_string(), "2026-01-05");
    assert_eq!(Date::parse("2026-02-30"), None);
    assert_eq!(Date::parse("2026/02/03"), None);
    assert_eq!(Date::parse("2026-02"), None);
    assert_eq!(Date::parse("2026-02-03-04"), None);
    assert_eq!(Date::parse("x-02-03"), None);
    assert!(d(2026, 1, 5) < d(2026, 2, 1));
  }

  #[test]
  fn times_read_and_write_both_clocks() {
    let t = Time::new(14, 5, 0).unwrap();
    assert_eq!(t.to_string(), "14:05");
    assert_eq!(Time::new(9, 5, 7).unwrap().to_string(), "09:05:07");
    assert_eq!(Time::parse("14:05"), Some(t));
    assert_eq!(Time::parse("2:05 pm"), Some(t));
    assert_eq!(Time::parse("12:00 am"), Some(Time::MIDNIGHT));
    assert_eq!(Time::parse("12:30PM"), Time::new(12, 30, 0));
    assert_eq!(Time::parse("9:05:07"), Time::new(9, 5, 7));
    assert_eq!(t.hour12(), (2, true));
    assert_eq!(Time::MIDNIGHT.hour12(), (12, false));
    for bad in [
      "24:00", "12:60", "13:00 pm", "0:10 am", "12", "1:2:3:4", "a:00",
    ] {
      assert_eq!(Time::parse(bad), None, "{bad:?}");
    }
    assert!(Time::new(9, 0, 0).unwrap() < t);
    assert_eq!(t.seconds_since_midnight(), 50_700);
  }

  #[test]
  fn today_is_the_date_a_test_sets() {
    set_today(Some(d(2026, 1, 15)));
    assert_eq!(today(), d(2026, 1, 15));
    set_today(None);
    set_date_source(Some(Rc::new(|| Date::new(2030, 6, 1))));
    assert_eq!(today(), d(2030, 6, 1));
    set_date_source(None);
    assert!(today().year() >= 2026, "the system's date");
  }
}
