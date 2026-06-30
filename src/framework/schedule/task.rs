use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::framework::console::{Args, ConsoleKernel};

use super::error::TaskError;

pub type TaskFuture = Pin<Box<dyn Future<Output = Result<(), TaskError>> + Send>>;

pub trait ScheduledTask: Send + Sync {
    fn id(&self) -> &str;
    fn execute(&self, kernel: Arc<ConsoleKernel>) -> TaskFuture;
}

pub struct CommandTask {
    pub command_name: String,
    pub raw_args: Vec<String>,
}

impl ScheduledTask for CommandTask {
    fn id(&self) -> &str {
        &self.command_name
    }

    fn execute(&self, kernel: Arc<ConsoleKernel>) -> TaskFuture {
        let name = self.command_name.clone();
        let raw_args = self.raw_args.clone();

        Box::pin(async move {
            let args = Args::parse(raw_args);

            kernel.dispatch(&name, args).await.map_err(|e| TaskError {
                id: name.clone(),
                message: e.to_string(),
            })
        })
    }
}

pub struct ClosureTask {
    pub id: String,
    pub f: Arc<dyn Fn() -> TaskFuture + Send + Sync>,
}

impl ScheduledTask for ClosureTask {
    fn id(&self) -> &str {
        &self.id
    }

    fn execute(&self, _kernel: Arc<ConsoleKernel>) -> TaskFuture {
        (self.f)()
    }
}
