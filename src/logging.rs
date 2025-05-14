use log::LevelFilter;
use env_logger::Builder;

/// Initializes the logger with a default log level of `Info`.
pub fn init_logger() {
    Builder::new()
        .filter_level(LevelFilter::Info)
        .format_timestamp(None)
        .init();
}
