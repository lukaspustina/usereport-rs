use crate::command::CommandResult;

use spectral::{AssertionFailure, Spec};

/// Initialize environment for tests, e.g. Logging.
pub(crate) fn init() {
    let _ = env_logger::builder().is_test(true).try_init();
}

pub(crate) trait CommandResultSuccess {
    fn is_success_contains(&mut self, expected: &str);
    fn is_failed(&mut self);
    fn is_timeout(&mut self);
}

impl<'s> CommandResultSuccess for Spec<'s, CommandResult> {
    fn is_success_contains(&mut self, expected: &str) {
        let subject = self.subject;
        match subject {
            CommandResult::Success(_, x) if x.contains(expected) => {},
            _ => AssertionFailure::from_spec(self)
                .with_expected(format!("command result is success and contains '{}'", expected))
                .with_actual(format!("'{:?}'", subject))
                .fail()
        }
    }

    fn is_failed(&mut self) {
        let subject = self.subject;
        match subject {
            CommandResult::Failed(_) => {},
            _ => AssertionFailure::from_spec(self)
                .with_expected(format!("command result is failed"))
                .with_actual(format!("'{:?}'", subject))
                .fail()
        }
    }

    fn is_timeout(&mut self) {
        let subject = self.subject;
        match subject {
            CommandResult::Timeout(_) => {},
            _ => AssertionFailure::from_spec(self)
                .with_expected(format!("command result is failed"))
                .with_actual(format!("'{:?}'", subject))
                .fail()
        }
    }
}
