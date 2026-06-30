use chrono::Weekday;

/// A schedule frequency. Each variant maps to a 6-field cron expression
/// (`second minute hour day-of-month month day-of-week`) via
/// [`to_cron_expr`](Frequency::to_cron_expr). `Raw` holds a user-supplied
/// expression passed through verbatim (5 or 6 fields; seconds optional).
pub enum Frequency {
    Raw(String),
    EverySecond,
    EveryMinutes(u32),
    HourlyAt(u32),
    DailyAt {
        hour: u32,
        minute: u32,
    },
    TwiceDaily(u32, u32),
    WeeklyOn {
        day: Weekday,
        hour: u32,
        minute: u32,
    },
    MonthlyOn {
        day: u32,
        hour: u32,
        minute: u32,
    },
    Quarterly,
    Yearly,
}

impl Frequency {
    pub fn to_cron_expr(&self) -> String {
        match self {
            Frequency::Raw(s) => s.clone(),
            Frequency::EverySecond => "* * * * * *".to_string(),
            Frequency::EveryMinutes(1) => "0 * * * * *".to_string(),
            Frequency::EveryMinutes(n) => format!("0 */{n} * * * *"),
            Frequency::HourlyAt(min) => format!("0 {min} * * * *"),
            Frequency::DailyAt { hour, minute } => format!("0 {minute} {hour} * * *"),
            Frequency::TwiceDaily(h1, h2) => format!("0 0 {h1},{h2} * * *"),
            Frequency::WeeklyOn { day, hour, minute } => {
                let d = weekday_num(*day);
                format!("0 {minute} {hour} * * {d}")
            }
            Frequency::MonthlyOn { day, hour, minute } => {
                format!("0 {minute} {hour} {day} * *")
            }
            Frequency::Quarterly => "0 0 0 1 1,4,7,10 *".to_string(),
            Frequency::Yearly => "0 0 0 1 1 *".to_string(),
        }
    }
}

pub(crate) fn weekday_num(day: Weekday) -> u8 {
    match day {
        Weekday::Sun => 0,
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 3,
        Weekday::Thu => 4,
        Weekday::Fri => 5,
        Weekday::Sat => 6,
    }
}
