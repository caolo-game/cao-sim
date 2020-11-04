use caolo_sim::tables::{btree::BTreeTable, dense::DenseVecTable, JoinIterator};
use caolo_sim::{indices::EntityId, tables::flag::SparseFlagTable};
use criterion::{black_box, criterion_group, Criterion};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};

fn get_rand() -> impl rand::Rng {
    SmallRng::seed_from_u64(0xdeadbeef)
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct Flag {}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
struct LargeComponent {
    _a: [u8; 10],
    _b: [u8; 10],
    _c: [u8; 10],
    _d: [u8; 10],
    _e: [u8; 10],
    _f: [u8; 10],
}

fn random_vec_table(len: usize, domain: u32) -> DenseVecTable<EntityId, LargeComponent> {
    let mut rng = get_rand();
    let mut table = DenseVecTable::with_capacity(domain as usize);
    for _ in 0..len {
        let mut res = false;
        while !res {
            let id = EntityId(rng.gen_range(0, domain));
            res = table.insert_or_update(id, LargeComponent::default());
        }
    }
    table
}

fn random_bt_table(len: usize, domain: u32) -> BTreeTable<EntityId, LargeComponent> {
    let mut rng = get_rand();
    let mut table = BTreeTable::new();
    for _ in 0..len {
        let mut res = false;
        while !res {
            let id = EntityId(rng.gen_range(0, domain));
            res = table.insert_or_update(id, LargeComponent::default());
        }
    }
    table
}

fn join_vec_btree_2pow15_sparse(c: &mut Criterion) {
    c.bench_function("join_vec_btree_2pow15_sparse", |b| {
        let bt = random_bt_table(1 << 15, 1 << 16);
        let ve = random_vec_table(1 << 15, 1 << 16);
        b.iter(move || {
            let it = JoinIterator::new(ve.iter(), bt.iter());
            for joined in it {
                black_box(joined);
            }
        });
    });
}

fn join_btree_vec_2pow15_sparse(c: &mut Criterion) {
    c.bench_function("join_btree_vec_2pow15_sparse", |b| {
        let bt = random_bt_table(1 << 15, 1 << 16);
        let ve = random_vec_table(1 << 15, 1 << 16);
        b.iter(move || {
            let it = JoinIterator::new(bt.iter(), ve.iter());
            for joined in it {
                black_box(joined);
            }
        });
    });
}

fn join_vec_vec_2pow15_sparse(c: &mut Criterion) {
    c.bench_function("join_vec_vec_2pow15_sparse", |b| {
        let ta = random_vec_table(1 << 15, 1 << 16);
        let tb = random_vec_table(1 << 15, 1 << 16);
        b.iter(move || {
            let it = JoinIterator::new(tb.iter(), ta.iter());
            for joined in it {
                black_box(joined);
            }
        });
    });
}

fn join_bt_bt_2pow15_sparse(c: &mut Criterion) {
    c.bench_function("join_bt_bt_2pow15_sparse", |b| {
        let ta = random_bt_table(1 << 15, 1 << 16);
        let tb = random_bt_table(1 << 15, 1 << 16);
        b.iter(move || {
            let it = JoinIterator::new(tb.iter(), ta.iter());
            for joined in it {
                black_box(joined);
            }
        });
    });
}

fn join_vec_btree_2pow15_dense(c: &mut Criterion) {
    c.bench_function("join_vec_btree_2pow15_dense", |b| {
        let bt = random_bt_table(1 << 15, 1 << 15);
        let ve = random_vec_table(1 << 15, 1 << 15);
        b.iter(move || {
            let it = JoinIterator::new(ve.iter(), bt.iter());
            for joined in it {
                black_box(joined);
            }
        });
    });
}

fn join_btree_vec_2pow15_dense(c: &mut Criterion) {
    c.bench_function("join_btree_vec_2pow15_dense", |b| {
        let bt = random_bt_table(1 << 15, 1 << 15);
        let ve = random_vec_table(1 << 15, 1 << 15);
        b.iter(move || {
            let it = JoinIterator::new(bt.iter(), ve.iter());
            for joined in it {
                black_box(joined);
            }
        });
    });
}

fn join_vec_vec_2pow15_dense(c: &mut Criterion) {
    c.bench_function("join_vec_vec_2pow15_dense", |b| {
        let ta = random_vec_table(1 << 15, 1 << 15);
        let tb = random_vec_table(1 << 15, 1 << 15);
        b.iter(move || {
            let it = JoinIterator::new(tb.iter(), ta.iter());
            for joined in it {
                black_box(joined);
            }
        });
    });
}

fn join_bt_bt_2pow15_dense(c: &mut Criterion) {
    c.bench_function("join_bt_bt_2pow15_dense", |b| {
        let ta = random_bt_table(1 << 15, 1 << 15);
        let tb = random_bt_table(1 << 15, 1 << 15);
        b.iter(move || {
            let it = JoinIterator::new(tb.iter(), ta.iter());
            for joined in it {
                black_box(joined);
            }
        });
    });
}

fn join_flag_vec_sparse(c: &mut Criterion) {
    c.bench_function("join_flag_vec", |b| {
        let vectable = random_vec_table(1 << 12, 1 << 15);
        let mut flags = SparseFlagTable::<_, Flag>::default();

        for (id, _) in vectable.iter() {
            flags.insert(id);
        }

        b.iter(move || {
            let it = JoinIterator::new(flags.iter(), vectable.iter());
            for joined in it {
                black_box(joined);
            }
        });
    });
}

criterion_group!(
    join_benches,
    join_bt_bt_2pow15_dense,
    join_vec_vec_2pow15_dense,
    join_btree_vec_2pow15_dense,
    join_bt_bt_2pow15_dense,
    join_vec_btree_2pow15_dense,
    join_vec_vec_2pow15_sparse,
    join_btree_vec_2pow15_sparse,
    join_vec_btree_2pow15_sparse,
    join_bt_bt_2pow15_sparse,
    join_flag_vec_sparse
);
