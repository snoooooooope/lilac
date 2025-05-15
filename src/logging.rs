use log::LevelFilter;
use env_logger::Builder;
use std::io::Write;

/// Initializes the logger with a default log level of `Info`.
pub fn init_logger() {
    Builder::new()
        .filter_level(LevelFilter::Info)
        .format(|buf, record| {
            writeln!(
                buf,
                "{}", // Only the log message, no module path or timestamp
                record.args()
            )
        })
        .init();
}
