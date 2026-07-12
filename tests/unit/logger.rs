//! Demonstrates the actual payoff of routing `Logger` through `framework::di`
//! instead of a static facade: a test can bind a fake in place of
//! `TracingLogger` with zero production-code changes, then assert on what
//! was logged.

use std::sync::{Arc, Mutex};

use kolas::framework::di::ContainerBuilder;
use kolas::framework::logging::Logger;

struct FakeLogger {
    messages: Arc<Mutex<Vec<String>>>,
}

impl Logger for FakeLogger {
    fn debug(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(format!("DEBUG {message}"));
    }

    fn info(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(format!("INFO {message}"));
    }

    fn warn(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(format!("WARN {message}"));
    }

    fn error(&self, message: &str) {
        self.messages
            .lock()
            .unwrap()
            .push(format!("ERROR {message}"));
    }
}

#[tokio::test]
async fn fake_logger_can_be_substituted_for_the_default_binding() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let container = ContainerBuilder::new()
        .singleton::<dyn Logger>(Arc::new(FakeLogger {
            messages: Arc::clone(&captured),
        }))
        .build();

    let logger = container.resolve_in::<dyn Logger>().await.unwrap();
    logger.info("service started");
    logger.error("boom");

    let messages = captured.lock().unwrap();
    assert_eq!(&messages[..], &["INFO service started", "ERROR boom"]);
}
