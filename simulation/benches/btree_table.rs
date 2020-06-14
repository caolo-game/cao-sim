use caolo_sim::model::EntityId;
use caolo_sim::tables::btree::BTreeTable;
use criterion::{black_box, criterion_group, Criterion};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use std::convert::TryFrom;

fn get_rand() -> impl rand::Rng {
    SmallRng::seed_from_u64(0xdeadbeef)
}

fn insert_at_random(c: &mut Criterion) {
    c.bench_function("btree_table insert_at_random", |b| {
        let mut rng = get_rand();
        let mut table = BTreeTable::<EntityId, i32>::new();
        b.iter(|| {
            let id = rng.gen_range(0, 1 << 20);
            let id = EntityId(id);
            let res = table.insert_or_update(id, rng.gen_range(0, 200));
            debug_assert!(res);
            res
        });
    });
}

fn get_by_id_random_2_pow_16(c: &mut Criterion) {
    c.bench_function("btree_table get_by_id_random_2_pow_16", |b| {
        const LEN: usize = 1 << 16;
        let mut rng = get_rand();
        let mut table = BTreeTable::<EntityId, _>::new();
        for i in 0..LEN {
            let mut res = false;
            while !res {
                let id = rng.gen_range(0, 1 << 25);
                let id = EntityId(id);
                res = table.insert_or_update(id, i);
            }
        }
        b.iter(|| {
            let id = rng.gen_range(0, 1 << 25);
            let id = EntityId(id);
            let res = table.get_by_id(&id);
            res
        });
    });
}

fn update_all_iter_2pow14_sparse(c: &mut Criterion) {
    c.bench_function("btree_table update_all_iter_2pow14_sparse", |b| {
        // The Id domain is 1.2 * LEN

        const LEN: usize = 1 << 14;
        let mut rng = get_rand();
        let mut table = BTreeTable::<EntityId, usize>::new();
        for i in 0..LEN {
            let mut id = Default::default();
            while table.contains(&id) {
                id = EntityId(rng.gen_range(
                    0,
                    u32::try_from(LEN * 6 / 5).expect("max len to fit into u32"),
                ));
            }
            table.insert_or_update(id, i);
        }
        b.iter(|| {
            table.iter_mut().for_each(|(_, val)| {
                *val += 8;
                black_box(val);
            });
        });
    });
}

fn update_all_iter_2pow14_dense(c: &mut Criterion) {
    c.bench_function("btree_table update_all_iter_2pow14_dense", |b| {
        // The whole table is filled

        const LEN: usize = 1 << 14;
        let mut table = BTreeTable::<EntityId, usize>::new();
        for i in 0..LEN {
            let id = EntityId(i as u32);
            table.insert_or_update(id, i);
        }
        b.iter(|| {
            table.iter_mut().for_each(|(_, val)| {
                *val += 8;
                black_box(val);
            });
        });
    });
}

criterion_group!(
    btree_benches,
    update_all_iter_2pow14_dense,
    update_all_iter_2pow14_sparse,
    insert_at_random,
    get_by_id_random_2_pow_16,
);
