#[cfg(test)]
use std::sync::Once;
#[cfg(test)]
static INIT: Once = Once::new();

#[cfg(test)]
pub fn setup_testing() {
    INIT.call_once(|| {
        let log_lvl = std::env::var("RUST_LOG").unwrap_or_else(|_| "".to_owned());
        std::env::set_var(
            "RUST_LOG",
            &format!("{},caolo_sim::storage::views=trace", log_lvl),
        );
        env_logger::init();
    });
}

#[cfg(test)]
pub fn test_logger() -> slog::Logger {
    use slog::{o, Drain, Logger};
    Logger::root(slog_stdlog::StdLog.fuse(), o!())
}

/// If `profile` feature is enabled, records high-level profiling information to `profile.csv`.
/// Recording is done via a thread-local buffer and dedicated file writing thread, in an attempt to
/// mitigate overhead.
///
#[macro_export(local_inner_macros)]
macro_rules! profile {
    ($name: expr) => {
        #[cfg(feature = "profile")]
        cao_profile::profile!($name)
    };
    (trace $name: expr) => {
        log::trace!($name);
        #[cfg(feature = "profile")]
        cao_profile::profile!($name)
    };
}
