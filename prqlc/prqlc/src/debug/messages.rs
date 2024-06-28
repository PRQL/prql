use log::{Metadata, Record};

use crate::debug;

pub struct MessageLogger;

impl log::Log for MessageLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        // if log is enabled, enable all message levels
        super::log_is_enabled()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            super::log_entry(|| {
                debug::DebugEntryKind::Message(debug::Message {
                    level: record.level().to_string(),
                    file: record.file().map(|x| x.to_string()),
                    line: record.line(),
                    module_path: record.module_path().map(|x| x.to_string()),
                    text: format!("{}", record.args()),
                })
            });
        }
    }

    fn flush(&self) {}
}
