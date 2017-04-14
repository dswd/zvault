use log::{self, LogRecord, LogLevel, LogMetadata};
pub use log::SetLoggerError;

use ansi_term::{Color, Style};
use std::io::Write;


macro_rules! println_stderr(
    ($($arg:tt)*) => { {
        let r = writeln!(&mut ::std::io::stderr(), $($arg)*);
        r.expect("failed printing to stderr");
    } }
);

struct Logger(LogLevel);

impl log::Log for Logger {
    fn enabled(&self, metadata: &LogMetadata) -> bool {
        metadata.level() <= self.0
    }

    fn log(&self, record: &LogRecord) {
        if self.enabled(record.metadata()) {
            match record.level() {
                LogLevel::Error => println_stderr!("{}: {}", Color::Red.bold().paint("error"), record.args()),
                LogLevel::Warn => println_stderr!("{}: {}", Color::Yellow.bold().paint("warning"), record.args()),
                LogLevel::Info => println_stderr!("{}: {}", Color::Green.bold().paint("info"), record.args()),
                LogLevel::Debug => println_stderr!("{}: {}", Style::new().bold().paint("debug"), record.args()),
                LogLevel::Trace => println_stderr!("{}: {}", "trace", record.args())
            }
        }
    }
}

pub fn init(level: LogLevel) -> Result<(), SetLoggerError> {
    log::set_logger(|max_log_level| {
        max_log_level.set(level.to_log_level_filter());
        Box::new(Logger(level))
    })
}
