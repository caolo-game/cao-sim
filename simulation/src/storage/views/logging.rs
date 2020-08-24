use lazy_static::lazy_static;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use slog::{error, Logger};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogGuard {
    pub fname: String,
    pub logger: Logger,
}

impl Drop for LogGuard {
    fn drop(&mut self) {
        // ensure only one write at a time, so if a client messes up and creates multiple guards
        // with the same name it'll function at least..
        let _guard = SAVE_TEX.lock().expect("Failed to aquire SAVE_TEX");
        let table = TABLE_LOG_HISTORY
            .lock()
            .expect("Failed to aquire TABLE_LOG_HISTORY");
        if let Ok(f) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.fname)
        {
            let table = table
                .iter()
                .map(|(k, logger)| {
                    let history = logger.history.lock().expect("LogGuard drop history mutex");
                    let mut history = history
                        .iter()
                        .filter(|(_, value)| !value.is_null())
                        .collect::<Vec<_>>();
                    history.sort_unstable_by_key(|(i, _)| i);
                    let history = serde_json::to_value(&history).unwrap();
                    (k, history)
                })
                .collect::<HashMap<_, _>>();
            serde_json::to_writer_pretty(f, &table)
                .map_err(|e| {
                    error!(self.logger, "Failed to write table history {:?}", e);
                })
                .unwrap_or(());
        } else {
            error!(
                self.logger,
                "Failed to open table logging file: {}", self.fname
            );
        }
    }
}

#[derive(Debug)]
pub struct TableLog {
    history: Mutex<Pin<Box<[(usize, Value)]>>>,
    next: AtomicUsize,
}

impl Default for TableLog {
    fn default() -> Self {
        Self {
            history: Mutex::new(Pin::new(
                vec![Default::default(); MAX_LOG_HISTORY].into_boxed_slice(),
            )),
            next: AtomicUsize::new(0),
        }
    }
}

unsafe impl Send for TableLog {}
unsafe impl Sync for TableLog {}

impl TableLog {
    /// # Safety
    /// as long as MAX_LOG_HISTORY is larger than the number of threads accessing this table
    /// we're most likely fine, but this is pretty unsafe
    pub unsafe fn inserter<'a>(&'a self) -> impl FnOnce(Value) -> () + 'a {
        let i = self.next.fetch_add(1, Ordering::AcqRel);
        let history = &self.history;
        move |value| {
            let j = i % MAX_LOG_HISTORY;
            let p = &mut history.lock().expect("TableLog insert")[j];
            *p = (i, value);
        }
    }
}

pub const MAX_LOG_HISTORY: usize = 32;

lazy_static! {
    pub static ref TABLE_LOG_HISTORY: Mutex<HashMap<&'static str, TableLog>> = {
        let map = HashMap::with_capacity(32);
        Mutex::new(map)
    };
    pub static ref NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    static ref SAVE_TEX: Mutex<()> = Mutex::new(());
}

#[cfg(test)]
mod tests {
    use super::super::UnsafeView;
    use super::*;
    use crate::components::EntityComponent;
    use crate::geometry::Axial;
    use crate::indices::EntityId;
    use crate::tables::morton::MortonTable;
    use crate::tables::traits::Table;

    type TestTable = MortonTable<Axial, EntityComponent>;

    #[test]
    fn saves_log() {
        crate::utils::setup_testing();

        let p = Axial::new(1, 2);
        let mut table: TestTable = MortonTable::from_iterator((0..16).map(move |i| {
            let p = p * i;
            (p, EntityComponent(EntityId(i as u32)))
        }))
        .unwrap();

        let logger = crate::utils::test_logger();

        {
            let _log_guard = LogGuard {
                fname: "test-tables.log".to_owned(),
                logger,
            };

            {
                let v: UnsafeView<Axial, EntityComponent> = UnsafeView::from_table(&mut table);
                v.log_table();
            }
        }

        let f = OpenOptions::new()
            .create(false)
            .read(true)
            .truncate(false)
            .open("test-tables.log")
            .unwrap();

        let mut dict: serde_json::Value = serde_json::from_reader(&f).unwrap();

        assert!(dict.is_object());

        let id = TestTable::name();
        let hist = dict[&id].take();

        let read_tables: Vec<(usize, TestTable)> =
            serde_json::from_value(hist).expect("morton deser");

        assert!(!read_tables.is_empty());

        for (expected, actual) in read_tables[0].1.iter().zip(table.iter()) {
            assert_eq!(expected, actual);
        }
    }
}
