use log::{self, LogRecord, LogLevel, LogMetadata};
pub use log::SetLoggerError;

use ansi_term::{Color, Style};


struct Logger(LogLevel);

impl log::Log for Logger {
    fn enabled(&self, metadata: &LogMetadata) -> bool {
        metadata.level() <= self.0
    }

    fn log(&self, record: &LogRecord) {
        if self.enabled(record.metadata()) {
            match record.level() {
                LogLevel::Error => println!("{}: {}", Color::Red.bold().paint("error"), record.args()),
                LogLevel::Warn => println!("{}: {}", Color::Yellow.bold().paint("warning"), record.args()),
                LogLevel::Info => println!("{}: {}", Color::Green.bold().paint("info"), record.args()),
                LogLevel::Debug => println!("{}: {}", Style::new().bold().paint("debug"), record.args()),
                LogLevel::Trace => println!("{}: {}", "trace", record.args())
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
