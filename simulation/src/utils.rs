#![allow(unused)]

use std::sync::Once;
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

#[macro_export(local_inner_macros)]
macro_rules! profile {
    ($name: expr) => {
        #[cfg(feature = "profile")]
        let _profile = {
            use crate::utils::profiler::Profiler;

            Profiler::new(std::file!(), std::line!(), $name)
        };
    };
    (trace $name: expr) => {
        log::trace!($name);
        #[cfg(feature = "profile")]
        let _profile = {
            use crate::utils::profiler::Profiler;

            Profiler::new(std::file!(), std::line!(), $name)
        };
    };
}

/// If `profile` feature is enabled, records profiling information to `profile.csv`.
/// Recording is done via a thread-local buffer and dedicated file writing thread, in an attempt to
/// mitigate overhead.
///
pub mod profiler {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::fs::File;
    use std::sync::mpsc::{channel, Sender};
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    #[cfg(feature = "profile")]
    lazy_static::lazy_static! {
        static ref COMM: Mutex<Aggregate> = {
            let (sender, receiver) = channel::<Vec<Record<'static>>>();
            let worker = std::thread::spawn(move || {
                while let Ok(rows) = receiver.recv() {
                    use std::fs::File;
                    use std::io::Write;

                    let mut file = PROF_FILE.lock().unwrap();

                    for row in rows {
                        writeln!(
                            file,
                            "[{}::{}::{}],{},ns",
                            row.file,
                            row.line,
                            row.name,
                            row.duration.as_nanos()
                        );
                    }
                }
            });
            let res = Aggregate {
                sender, worker
            };
            Mutex::new(res)
        };
        static ref PROF_FILE: Mutex<std::fs::File> = {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .append(true)
                .open("profile.csv")
                .expect("profiler file");
            Mutex::new(file)
        };
    }

    #[cfg(feature = "profile")]
    thread_local!(
        static LOCAL_COMM: RefCell<LocalAggregate> = {
            let comm = COMM.lock().unwrap();
            let sender = comm.get_sender();
            let res = LocalAggregate {
                sender,
                container: Vec::with_capacity(1 << 15),
            };
            RefCell::new(res)
        };
    );

    #[cfg(feature = "profile")]
    struct Aggregate {
        worker: std::thread::JoinHandle<()>,
        sender: Sender<Vec<Record<'static>>>,
    }

    #[cfg(feature = "profile")]
    struct LocalAggregate {
        sender: Sender<Vec<Record<'static>>>,
        container: Vec<Record<'static>>,
    }

    #[cfg(feature = "profile")]
    impl LocalAggregate {
        pub fn push(&mut self, r: Record<'static>) {
            self.container.push(r);
            if self.container.len() >= ((1 << 15) - 1) {
                let mut v = Vec::with_capacity(1 << 15);
                std::mem::swap(&mut v, &mut self.container);
                self.sender.send(v);
            }
        }

        fn save<'a>(v: &[Record<'a>]) {
            use std::fs::File;
            use std::io::Write;

            let mut file = PROF_FILE.lock().unwrap();

            for row in v.iter() {
                writeln!(
                    file,
                    "[{}::{}::{}],{},ns",
                    row.file,
                    row.line,
                    row.name,
                    row.duration.as_nanos()
                );
            }
        }
    }

    #[cfg(feature = "profile")]
    impl Aggregate {
        pub fn get_sender(&self) -> Sender<Vec<Record<'static>>> {
            self.sender.clone()
        }
    }

    #[cfg(feature = "profile")]
    impl Drop for LocalAggregate {
        fn drop(&mut self) {
            Self::save(&self.container);
        }
    }

    struct Record<'a> {
        duration: Duration,
        name: &'a str,
        file: &'a str,
        line: u32,
    }

    /// Output execution of it's scope.
    /// Output is in CSV format: name, time, timeunit
    pub struct Profiler {
        start: Instant,
        name: &'static str,
        file: &'static str,
        line: u32,
    }

    impl Profiler {
        pub fn new(file: &'static str, line: u32, name: &'static str) -> Self {
            let start = Instant::now();
            Self {
                name,
                start,
                file,
                line,
            }
        }
    }

    impl Drop for Profiler {
        fn drop(&mut self) {
            let end = Instant::now();
            let dur = end - self.start;
            let mil = dur.as_millis();

            #[cfg(feature = "profile")]
            {
                LOCAL_COMM.with(|comm| {
                    comm.borrow_mut().push(Record {
                        name: self.name,
                        file: self.file,
                        line: self.line,
                        duration: dur,
                    })
                });
            }
        }
    }
}
