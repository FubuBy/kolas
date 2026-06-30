use chrono::{DateTime, Duration as ChronoDuration, Utc, Weekday};
use chrono_tz::Tz;
use croner::Cron;
use croner::parser::{CronParser, Seconds};

use super::error::ScheduleError;
use super::frequency::{Frequency, weekday_num};
use super::task::ScheduledTask;

/// A single scheduled entry: a task plus its cron frequency and options.
///
/// Every builder method returns `&mut Self` so calls chain fluently. Methods
/// that take fallible input (`daily_at`, `monthly_on`, `between`, `timezone`)
/// do not return a `Result`; instead they record the first error, which
/// surfaces when the event is compiled (`build_cron`/`expression`/`is_due`) or
/// when the schedule is validated. This keeps registration in
/// `app/console/schedule.rs` free of `?` noise.
pub struct Event {
    pub(crate) task: Box<dyn ScheduledTask>,
    pub(crate) frequency: Frequency,
    pub(crate) timezone: Option<Tz>,
    pub(crate) without_overlapping: bool,
    pub(crate) on_one_server: bool,
    pub(crate) dow_filter: Option<DowFilter>,
    pub(crate) between: Option<(u32, u32)>,
    pub(crate) error: Option<ScheduleError>,
}

/// Day-of-week restriction layered on top of the base frequency.
pub(crate) enum DowFilter {
    Weekdays,
    Weekends,
    Single(Weekday),
}

impl Event {
    pub(crate) fn new(task: Box<dyn ScheduledTask>) -> Self {
        Self {
            task,
            frequency: Frequency::EveryMinutes(1),
            timezone: None,
            without_overlapping: false,
            on_one_server: false,
            dow_filter: None,
            between: None,
            error: None,
        }
    }

    /// Records the first error encountered while building the event.
    fn fail(&mut self, error: ScheduleError) -> &mut Self {
        if self.error.is_none() {
            self.error = Some(error);
        }
        self
    }

    pub fn cron(&mut self, expr: &str) -> &mut Self {
        self.frequency = Frequency::Raw(expr.to_string());
        self
    }

    pub fn every_second(&mut self) -> &mut Self {
        self.frequency = Frequency::EverySecond;
        self
    }

    pub fn every_minute(&mut self) -> &mut Self {
        self.frequency = Frequency::EveryMinutes(1);
        self
    }

    pub fn every_minutes(&mut self, n: u32) -> &mut Self {
        self.frequency = Frequency::EveryMinutes(n);
        self
    }

    pub fn every_five_minutes(&mut self) -> &mut Self {
        self.every_minutes(5)
    }

    pub fn every_ten_minutes(&mut self) -> &mut Self {
        self.every_minutes(10)
    }

    pub fn every_fifteen_minutes(&mut self) -> &mut Self {
        self.every_minutes(15)
    }

    pub fn every_thirty_minutes(&mut self) -> &mut Self {
        self.frequency = Frequency::Raw("0 0,30 * * * *".to_string());
        self
    }

    pub fn hourly(&mut self) -> &mut Self {
        self.frequency = Frequency::HourlyAt(0);
        self
    }

    pub fn hourly_at(&mut self, minute: u32) -> &mut Self {
        if minute > 59 {
            return self.fail(ScheduleError::InvalidTimeFormat {
                value: format!("minute {minute}"),
            });
        }
        self.frequency = Frequency::HourlyAt(minute);
        self
    }

    pub fn daily(&mut self) -> &mut Self {
        self.frequency = Frequency::DailyAt { hour: 0, minute: 0 };
        self
    }

    pub fn daily_at(&mut self, time: &str) -> &mut Self {
        match parse_time(time) {
            Ok((hour, minute)) => {
                self.frequency = Frequency::DailyAt { hour, minute };
                self
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn twice_daily(&mut self, first: u32, second: u32) -> &mut Self {
        if first > 23 || second > 23 {
            return self.fail(ScheduleError::InvalidTimeFormat {
                value: format!("hours {first},{second}"),
            });
        }
        self.frequency = Frequency::TwiceDaily(first, second);
        self
    }

    pub fn weekly(&mut self) -> &mut Self {
        self.frequency = Frequency::WeeklyOn {
            day: Weekday::Sun,
            hour: 0,
            minute: 0,
        };
        self
    }

    pub fn weekly_on(&mut self, day: Weekday, time: &str) -> &mut Self {
        match parse_time(time) {
            Ok((hour, minute)) => {
                self.frequency = Frequency::WeeklyOn { day, hour, minute };
                self
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn monthly(&mut self) -> &mut Self {
        self.frequency = Frequency::MonthlyOn {
            day: 1,
            hour: 0,
            minute: 0,
        };
        self
    }

    pub fn monthly_on(&mut self, day: u32, time: &str) -> &mut Self {
        if !(1..=31).contains(&day) {
            return self.fail(ScheduleError::InvalidDay { day });
        }

        match parse_time(time) {
            Ok((hour, minute)) => {
                self.frequency = Frequency::MonthlyOn { day, hour, minute };
                self
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn quarterly(&mut self) -> &mut Self {
        self.frequency = Frequency::Quarterly;
        self
    }

    pub fn yearly(&mut self) -> &mut Self {
        self.frequency = Frequency::Yearly;
        self
    }

    pub fn weekdays(&mut self) -> &mut Self {
        self.dow_filter = Some(DowFilter::Weekdays);
        self
    }

    pub fn weekends(&mut self) -> &mut Self {
        self.dow_filter = Some(DowFilter::Weekends);
        self
    }

    pub fn mondays(&mut self) -> &mut Self {
        self.dow_filter = Some(DowFilter::Single(Weekday::Mon));
        self
    }

    pub fn tuesdays(&mut self) -> &mut Self {
        self.dow_filter = Some(DowFilter::Single(Weekday::Tue));
        self
    }

    pub fn wednesdays(&mut self) -> &mut Self {
        self.dow_filter = Some(DowFilter::Single(Weekday::Wed));
        self
    }

    pub fn thursdays(&mut self) -> &mut Self {
        self.dow_filter = Some(DowFilter::Single(Weekday::Thu));
        self
    }

    pub fn fridays(&mut self) -> &mut Self {
        self.dow_filter = Some(DowFilter::Single(Weekday::Fri));
        self
    }

    pub fn saturdays(&mut self) -> &mut Self {
        self.dow_filter = Some(DowFilter::Single(Weekday::Sat));
        self
    }

    pub fn sundays(&mut self) -> &mut Self {
        self.dow_filter = Some(DowFilter::Single(Weekday::Sun));
        self
    }

    /// Restricts execution to the hour range `[start, end]`. Only the hour
    /// component of each `HH:MM` value is used (minute precision is out of
    /// scope for this iteration).
    pub fn between(&mut self, start: &str, end: &str) -> &mut Self {
        let parsed = parse_time(start).and_then(|(sh, _)| parse_time(end).map(|(eh, _)| (sh, eh)));

        match parsed {
            Ok((sh, eh)) if sh > eh => self.fail(ScheduleError::InvalidTimeFormat {
                value: format!("{start}-{end} (start hour after end hour)"),
            }),
            Ok(range) => {
                self.between = Some(range);
                self
            }
            Err(e) => self.fail(e),
        }
    }

    pub fn timezone(&mut self, tz: &str) -> &mut Self {
        match tz.parse::<Tz>() {
            Ok(parsed) => {
                self.timezone = Some(parsed);
                self
            }
            Err(_) => self.fail(ScheduleError::InvalidTimezone { tz: tz.to_string() }),
        }
    }

    pub fn without_overlapping(&mut self) -> &mut Self {
        self.without_overlapping = true;
        self
    }

    pub fn on_one_server(&mut self) -> &mut Self {
        self.on_one_server = true;
        self
    }

    /// The fully composed cron expression (base frequency + day-of-week and
    /// hour-range modifiers). Returns the recorded build error, if any.
    pub fn expression(&self) -> Result<String, ScheduleError> {
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        Ok(self.apply_modifiers(self.frequency.to_cron_expr()))
    }

    /// Compiles the event into a parsed [`Cron`].
    pub fn build_cron(&self) -> Result<Cron, ScheduleError> {
        let expr = self.expression()?;
        CronParser::builder()
            .seconds(Seconds::Optional)
            .build()
            .parse(&expr)
            .map_err(|e| ScheduleError::InvalidCron {
                expr,
                message: e.to_string(),
            })
    }

    /// Whether this event is due within the minute ending at `now`, using its
    /// own timezone (falling back to UTC when none is set).
    pub fn is_due(&self, now: DateTime<Utc>) -> Result<bool, ScheduleError> {
        self.is_due_with(now, chrono_tz::UTC)
    }

    /// Like [`is_due`](Self::is_due) but uses `default_tz` when the event has no
    /// explicit timezone. Used by the runner so a schedule-wide default applies.
    pub(crate) fn is_due_with(
        &self,
        now: DateTime<Utc>,
        default_tz: Tz,
    ) -> Result<bool, ScheduleError> {
        let cron = self.build_cron()?;
        let tz = self.timezone.unwrap_or(default_tz);
        let local = now.with_timezone(&tz);
        let window_start = local - ChronoDuration::seconds(60);

        match cron.find_next_occurrence(&window_start, false) {
            Ok(next) => Ok(next <= local),
            Err(e) => Err(ScheduleError::NextOccurrence {
                expr: self.frequency.to_cron_expr(),
                message: e.to_string(),
            }),
        }
    }

    pub fn id(&self) -> &str {
        self.task.id()
    }

    /// Splices day-of-week and hour-range modifiers into a 6-field expression.
    /// Non-6-field (e.g. custom 5-field `cron(...)`) expressions are returned
    /// unchanged.
    fn apply_modifiers(&self, base: String) -> String {
        if self.dow_filter.is_none() && self.between.is_none() {
            return base;
        }

        let mut parts: Vec<String> = base.split_whitespace().map(String::from).collect();

        if parts.len() != 6 {
            return base;
        }

        if let Some((start, end)) = self.between {
            parts[2] = format!("{start}-{end}");
        }

        if let Some(filter) = &self.dow_filter {
            parts[5] = match filter {
                DowFilter::Weekdays => "1-5".to_string(),
                DowFilter::Weekends => "0,6".to_string(),
                DowFilter::Single(day) => weekday_num(*day).to_string(),
            };
        }
        parts.join(" ")
    }
}

fn parse_time(value: &str) -> Result<(u32, u32), ScheduleError> {
    let invalid = || ScheduleError::InvalidTimeFormat {
        value: value.to_string(),
    };
    let (h_str, m_str) = value.split_once(':').ok_or_else(invalid)?;
    let hour: u32 = h_str.parse().map_err(|_| invalid())?;
    let minute: u32 = m_str.parse().map_err(|_| invalid())?;

    if hour > 23 || minute > 59 {
        return Err(invalid());
    }
    Ok((hour, minute))
}
