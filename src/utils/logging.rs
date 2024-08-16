use chrono::Local;
use env_logger::Builder;
use log::{LevelFilter, SetLoggerError};
use std::io::Write;

pub fn init_logger() -> Result<(), SetLoggerError> {
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} [{}] - {}: {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .filter(None, LevelFilter::Info)
        .init();
    Ok(())
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => (log::info!($($arg)*))
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => (log::error!($($arg)*))
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => (log::warn!($($arg)*))
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => (log::debug!($($arg)*))
}
