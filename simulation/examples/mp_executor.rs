use caolo_sim::executor::mp_executor::MpExecutor;
use slog::{Drain, info, o};

fn main() {
    std::env::set_var("RUST_LOG", "info");

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_envlogger::new(drain).fuse();
    let drain = slog_async::Async::new(drain)
        .overflow_strategy(slog_async::OverflowStrategy::Block)
        .build()
        .fuse();
    let logger = slog::Logger::root(drain, o!());

    let mut executor = MpExecutor::new(logger.clone(), None).unwrap();
    loop {
        let role = executor.update_role().unwrap();
        info!(logger, "current role: {:?}", role);
        std::thread::sleep_ms(500);
    }
}
