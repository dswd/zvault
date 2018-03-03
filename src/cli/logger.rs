use log;
pub use log::SetLoggerError;

use ansi_term::{Color, Style};


struct Logger(log::Level);

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.0
    }

    fn flush(&self) {}

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            match record.level() {
                log::Level::Error => {
                    eprintln!("{}: {}", Color::Red.bold().paint("error"), record.args())
                }
                log::Level::Warn => {
                    eprintln!(
                        "{}: {}",
                        Color::Yellow.bold().paint("warning"),
                        record.args()
                    )
                }
                log::Level::Info => {
                    eprintln!("{}: {}", Color::Green.bold().paint("info"), record.args())
                }
                log::Level::Debug => {
                    eprintln!("{}: {}", Style::new().bold().paint("debug"), record.args())
                }
                log::Level::Trace => eprintln!("{}: {}", "trace", record.args()),
            }
        }
    }
}

pub fn init(level: log::Level) -> Result<(), SetLoggerError> {
    let logger = Logger(level);
    log::set_max_level(level.to_level_filter());
    log::set_boxed_logger(Box::new(logger))
}
