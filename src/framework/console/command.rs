use std::future::Future;
use std::pin::Pin;

use super::Args;

pub type BoxFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error>>> + Send + 'a>>;

pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn execute(&self, args: Args) -> BoxFuture<'_>;
}
