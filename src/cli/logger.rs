use log;
pub use log::SetLoggerError;

use ansi_term::{Color, Style};
use std::io::Write;


macro_rules! println_stderr(
    ($($arg:tt)*) => { {
        let r = writeln!(&mut ::std::io::stderr(), $($arg)*);
        r.expect("failed printing to stderr");
    } }
);

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
                    println_stderr!("{}: {}", Color::Red.bold().paint("error"), record.args())
                }
                log::Level::Warn => {
                    println_stderr!(
                        "{}: {}",
                        Color::Yellow.bold().paint("warning"),
                        record.args()
                    )
                }
                log::Level::Info => {
                    println_stderr!("{}: {}", Color::Green.bold().paint("info"), record.args())
                }
                log::Level::Debug => {
                    println_stderr!("{}: {}", Style::new().bold().paint("debug"), record.args())
                }
                log::Level::Trace => println_stderr!("{}: {}", "trace", record.args()),
            }
        }
    }
}

pub fn init(level: log::Level) -> Result<(), SetLoggerError> {
    let logger = Logger(level);
    log::set_max_level(level.to_level_filter());
    log::set_boxed_logger(Box::new(logger))
}
