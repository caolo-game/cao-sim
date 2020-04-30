mod btree_table;
mod morton_table;
mod table_join;
mod vec_table;

use criterion::criterion_main;

criterion_main!(
    morton_table::morton_benches,
    btree_table::btree_benches,
    table_join::join_benches,
    vec_table::vec_benches
);
