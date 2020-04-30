use caolo_sim::model::components::EntityComponent;
use caolo_sim::model::geometry::{Circle, Point};
use caolo_sim::model::EntityId;
use caolo_sim::tables::{MortonTable, PositionTable};
use criterion::{criterion_group, BenchmarkId, Criterion};
use rand::RngCore;
use rand::{rngs::SmallRng, Rng, SeedableRng};

fn get_rand() -> impl rand::Rng {
    SmallRng::seed_from_u64(0xdeadbeef)
}

fn contains_rand(c: &mut Criterion) {
    let mut group = c.benchmark_group("morton table contains_rand");
    for size in 8..16 {
        let size = 1 << size;
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, move |b, &size| {
            let mut rng = get_rand();

            let table = MortonTable::from_iterator((0..size).map(|i| {
                let p = Point {
                    x: rng.gen_range(0, 8000),
                    y: rng.gen_range(0, 8000),
                };
                (p, i)
            }))
            .unwrap();

            b.iter(|| {
                let p = Point {
                    x: rng.gen_range(0, 8000),
                    y: rng.gen_range(0, 8000),
                };
                table.contains_key(&p)
            })
        });
    }
    group.finish();
}

fn get_entities_in_range_sparse(c: &mut Criterion) {
    let mut group = c.benchmark_group("morton table get_entities_in_range sparse");
    for size in 8..16 {
        let size = 1 << size;
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut rng = get_rand();

            let table = MortonTable::from_iterator((0..size).map(|_| {
                let p = Point {
                    x: rng.gen_range(0, 3900 * 2),
                    y: rng.gen_range(0, 3900 * 2),
                };
                (p, EntityComponent(EntityId(rng.gen())))
            }))
            .unwrap();

            let radius = 512;
            b.iter(|| {
                let table = &table;
                let p = Point {
                    x: rng.gen_range(0, 3900 * 2),
                    y: rng.gen_range(0, 3900 * 2),
                };
                table.get_entities_in_range(&Circle { center: p, radius })
            });
        });
    }
    group.finish();
}

fn get_entities_in_range_dense(c: &mut Criterion) {
    let mut group = c.benchmark_group("morton table get_entities_in_range dense");
    for size in 8..16 {
        let size = 1 << size;
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut rng = get_rand();

            let table = MortonTable::from_iterator((0..size).map(|_| {
                let p = Point {
                    x: rng.gen_range(0, 200 * 2),
                    y: rng.gen_range(0, 200 * 2),
                };
                (p, EntityComponent(EntityId(rng.gen())))
            }))
            .unwrap();

            let radius = 50;
            b.iter(|| {
                let table = &table;
                let p = Point {
                    x: rng.gen_range(0, 200 * 2),
                    y: rng.gen_range(0, 200 * 2),
                };
                table.get_entities_in_range(&Circle { center: p, radius })
            });
        });
    }
    group.finish();
}

fn make_morton_table(c: &mut Criterion) {
    let mut group = c.benchmark_group("morton table make_morton_table");
    for size in 8..16 {
        let size = 1 << size;
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut rng = get_rand();

            b.iter(|| {
                let table = MortonTable::from_iterator((0..size).map(|_| {
                    (
                        Point {
                            x: rng.gen_range(0, 3900 * 2),
                            y: rng.gen_range(0, 3900 * 2),
                        },
                        rng.next_u32(),
                    )
                }))
                .unwrap();
                table
            });
        });
    }
    group.finish();
}

fn rebuild_morton_table(c: &mut Criterion) {
    let mut group = c.benchmark_group("morton table rebuild_morton_table");
    for size in 8..16 {
        let size = 1 << size;

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut rng = get_rand();

            let mut table = MortonTable::with_capacity(size);

            b.iter(|| {
                table.clear();

                table
                    .extend((0..size).map(|_| {
                        (
                            Point {
                                x: rng.gen_range(0, 3900 * 2),
                                y: rng.gen_range(0, 3900 * 2),
                            },
                            rng.next_u32(),
                        )
                    }))
                    .unwrap();
            });
        });
    }
    group.finish();
}

fn get_by_id_rand(c: &mut Criterion) {
    let mut group = c.benchmark_group("morton table get_by_id random");
    for size in 8..16 {
        let size = 1 << size;
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &len| {
            let mut rng = get_rand();

            let table = MortonTable::from_iterator((0..len).map(|_| {
                let pos = Point {
                    x: rng.gen_range(0, 3900 * 2),
                    y: rng.gen_range(0, 3900 * 2),
                };
                (pos, rng.next_u32())
            }))
            .unwrap();

            b.iter(|| {
                let pos = Point {
                    x: rng.gen_range(0, 3900 * 2),
                    y: rng.gen_range(0, 3900 * 2),
                };
                table.get_by_id(&pos)
            });
        });
    }
    group.finish();
}

fn get_by_id_in_table_rand(c: &mut Criterion) {
    let mut group =
        c.benchmark_group("morton table get_by_id, all queried elements are in the table");
    for size in 8..16 {
        let size = 1 << size;
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &len| {
            let mut rng = get_rand();

            let mut points = Vec::with_capacity(len);
            let table = MortonTable::from_iterator((0..len).map(|_| {
                let pos = Point {
                    x: rng.gen_range(0, 3900 * 2),
                    y: rng.gen_range(0, 3900 * 2),
                };
                points.push(pos.clone());
                (pos, rng.next_u32())
            }))
            .unwrap();

            b.iter(|| {
                let i = rng.gen_range(0, points.len());
                let pos = &points[i];
                table.get_by_id(pos)
            });
        });
    }
    group.finish();
}

fn random_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("morton table random_insert");
    for size in 8..14 {
        let size = 1 << size;
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            let mut rng = get_rand();
            let mut table = MortonTable::<Point, usize>::new();

            for _ in 0..size {
                let x = rng.gen_range(0, 29000);
                let y = rng.gen_range(0, 29000);
                let p = Point::new(x, y);

                table.insert(p, 420);
            }

            b.iter(|| {
                let x = rng.gen_range(0, 29000);
                let y = rng.gen_range(0, 29000);
                let p = Point::new(x, y);

                table.insert(p, 420)
            });
        });
    }
    group.finish();
}

criterion_group!(
    morton_benches,
    contains_rand,
    get_entities_in_range_sparse,
    get_entities_in_range_dense,
    make_morton_table,
    random_insert,
    rebuild_morton_table,
    get_by_id_in_table_rand,
    get_by_id_rand,
);
