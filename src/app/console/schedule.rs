//! Application schedule definition — the console analogue of `routes/api.rs`.
//!
//! Register periodic tasks here. Each task is either a registered console
//! command (run via the kernel or an arbitrary async closure. Frequencies chain fluently;
//! see `framework::schedule::Event` for the full set of builder methods.

use crate::framework::schedule::Schedule;

pub fn schedule(schedule: &mut Schedule) {
    // Run the `test` command every weekday at 09:00.
    schedule.command("test").daily_at("09:00").weekdays();

    // Run an inline async task every hour, skipping if the previous run is
    // still in progress.
    schedule
        .call(|| async {
            // ... your periodic logic here ...
            Ok(())
        })
        .hourly()
        .without_overlapping();
}
