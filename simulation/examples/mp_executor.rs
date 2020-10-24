use std::time::Duration;

use cao_lang::compiler::{compile, CompilationUnit, CompileOptions};
use caolo_sim::{
    components::EntityScript,
    components::ScriptComponent,
    executor::{mp_executor::MpExecutor, Executor},
    prelude::EntityId,
    prelude::ScriptId,
};
use slog::{debug, o, Drain};
use uuid::Uuid;

fn main() {
    std::env::set_var("RUST_LOG", "info,caolo_sim::executor::mp_executor=info");

    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_envlogger::new(drain).fuse();
    let drain = slog_async::Async::new(drain)
        .overflow_strategy(slog_async::OverflowStrategy::Block)
        .build()
        .fuse();
    let logger = slog::Logger::root(drain, o!());

    let mut executor = MpExecutor::new(logger.clone(), None).unwrap();
    let mut world = executor.initialize(Some(logger.clone())).unwrap();

    let script_id = ScriptId(Uuid::new_v4());
    let script: CompilationUnit =
        serde_json::from_str(include_str!("./program.json")).expect("deserialize example program");
    debug!(logger, "compiling default program");
    let compiled = compile(None, script, CompileOptions::new().with_breadcrumbs(false))
        .expect("failed to compile example program");

    caolo_sim::query!(
        mutate
        world
        {
            ScriptId, ScriptComponent,
                .insert_or_update(script_id, ScriptComponent(compiled));
        }
    );
    for _ in 0..6000 {
        let id = world.insert_entity();
        caolo_sim::query!(
            mutate
            world
            {
                EntityId, EntityScript,
                    .insert_or_update(id, EntityScript(script_id));
            }
        );
    }

    loop {
        executor.forward(&mut world).unwrap();
        std::thread::sleep(Duration::from_millis(500));
    }
}
