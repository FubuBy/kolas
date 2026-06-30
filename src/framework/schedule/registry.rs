use std::sync::Arc;

use chrono_tz::Tz;

use super::error::{ScheduleError, TaskError};
use super::event::Event;
use super::task::{ClosureTask, CommandTask, ScheduledTask, TaskFuture};

pub struct Schedule {
    pub(crate) events: Vec<Event>,
    pub(crate) default_timezone: Tz,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            default_timezone: chrono_tz::UTC,
        }
    }

    pub fn with_timezone(mut self, tz: Tz) -> Self {
        self.default_timezone = tz;
        self
    }

    pub fn command(&mut self, name: &str) -> &mut Event {
        let task = CommandTask {
            command_name: name.to_string(),
            raw_args: vec![],
        };
        self.push_event(Box::new(task))
    }

    pub fn command_with_args(&mut self, name: &str, raw_args: Vec<String>) -> &mut Event {
        let task = CommandTask {
            command_name: name.to_string(),
            raw_args,
        };
        self.push_event(Box::new(task))
    }

    pub fn call<F, Fut>(&mut self, f: F) -> &mut Event
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<(), TaskError>> + Send + 'static,
    {
        let id = format!("closure-{}", self.events.len());
        let arc_f = Arc::new(move || -> TaskFuture { Box::pin(f()) });
        let task = ClosureTask { id, f: arc_f };
        self.push_event(Box::new(task))
    }

    fn push_event(&mut self, task: Box<dyn ScheduledTask>) -> &mut Event {
        self.events.push(Event::new(task));
        self.events.last_mut().expect("event was just pushed")
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    pub fn validate(
        &self,
        kernel: &crate::framework::console::ConsoleKernel,
    ) -> Result<(), ScheduleError> {
        for event in &self.events {
            // Surfaces InvalidCron / InvalidTimeFormat / InvalidDay / InvalidTimezone
            // recorded while building the event.
            event.build_cron()?;

            let id = event.id();

            if id.starts_with("closure-") {
                continue;
            }

            if !kernel.has_command(id) {
                return Err(ScheduleError::UnknownCommand {
                    name: id.to_string(),
                });
            }
        }
        Ok(())
    }
}

impl Default for Schedule {
    fn default() -> Self {
        Self::new()
    }
}
