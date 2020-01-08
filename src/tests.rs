use crate::command::CommandResult;

use spectral::{AssertionFailure, Spec};

/// Initialize environment for tests, e.g. Logging.
pub(crate) fn init() { let _ = env_logger::builder().is_test(true).try_init(); }

pub(crate) trait CommandResultSuccess {
    fn is_success_contains(&mut self, expected: &str);
    fn is_failed(&mut self);
    fn is_timeout(&mut self);
    fn is_error_contains(&mut self, expected: &str);
}

impl<'s> CommandResultSuccess for Spec<'s, CommandResult> {
    fn is_success_contains(&mut self, expected: &str) {
        let subject = self.subject;
        match subject {
            CommandResult::Success { stdout: x, .. } if x.contains(expected) => {}
            _ => {
                AssertionFailure::from_spec(self)
                    .with_expected(format!("command result is success and contains '{}'", expected))
                    .with_actual(format!("'{:?}'", subject))
                    .fail()
            }
        }
    }

    fn is_failed(&mut self) {
        let subject = self.subject;
        match subject {
            CommandResult::Failed { .. } => {}
            _ => {
                AssertionFailure::from_spec(self)
                    .with_expected("command result is failed".to_string())
                    .with_actual(format!("'{:?}'", subject))
                    .fail()
            }
        }
    }

    fn is_timeout(&mut self) {
        let subject = self.subject;
        match subject {
            CommandResult::Timeout { .. } => {}
            _ => {
                AssertionFailure::from_spec(self)
                    .with_expected("command result is failed".to_string())
                    .with_actual(format!("'{:?}'", subject))
                    .fail()
            }
        }
    }

    fn is_error_contains(&mut self, expected: &str) {
        let subject = self.subject;
        match subject {
            CommandResult::Error { reason: x, .. } if x.contains(expected) => {}
            _ => {
                AssertionFailure::from_spec(self)
                    .with_expected("command result is error".to_string())
                    .with_actual(format!("'{:?}'", subject))
                    .fail()
            }
        }
    }
}
