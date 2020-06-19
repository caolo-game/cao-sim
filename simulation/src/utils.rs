#![allow(unused)]

#[macro_export(local_inner_macros)]
macro_rules! profile {
    ($name: expr) => {
        #[cfg(not(feature = "profile"))]
        log::trace!($name);
        #[cfg(feature = "profile")]
        let _profile = {
            use crate::utils::profiler::Profiler;

            Profiler::new(std::file!(), std::line!(), $name)
        };
    };
}

pub mod profiler {
    use std::collections::HashMap;
    use std::fs::File;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    #[cfg(feature = "profile")]
    lazy_static::lazy_static! {
        static ref COMM: Mutex<Aggregate> = {
            Mutex::new(
                Aggregate {
                    container : Vec::with_capacity(1<<17)
                }
            )
        };
        static ref PROF_FILE: Mutex<std::fs::File> = {
            let f = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .append(true)
                .open("profile.csv")
                .expect("profiler file");
            Mutex::new(f)
        };
    }

    #[cfg(feature = "profile")]
    struct Aggregate {
        container: Vec<Record<'static>>,
    }

    #[cfg(feature = "profile")]
    impl Aggregate {
        pub fn push(&mut self, r: Record<'static>) {
            self.container.push(r);
            if self.container.len() >= (1 << 16) {
                let mut v = Vec::with_capacity(1 << 17);
                std::mem::swap(&mut v, &mut self.container);
                std::thread::spawn(move || {
                    Self::save(&v);
                });
            }
        }

        fn save<'a>(v: &[Record<'a>]) {
            use std::fs::File;
            use std::io::Write;

            let mut f = PROF_FILE.lock().unwrap();

            for row in v.iter() {
                writeln!(
                    f,
                    "[{}::{}::{}],{},ns",
                    row.file,
                    row.line,
                    row.name,
                    row.duration.as_nanos()
                );
            }
        }
    }

    pub fn save_global() {
        #[cfg(feature = "profile")]
        {
            let mut c = COMM.lock().unwrap();
            Aggregate::save(&c.container);
            c.container.clear();
        }
    }

    #[cfg(feature = "profile")]
    impl Drop for Aggregate {
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
                COMM.lock().unwrap().push(Record {
                    name: self.name,
                    file: self.file,
                    line: self.line,
                    duration: dur,
                });
            }
        }
    }
}
