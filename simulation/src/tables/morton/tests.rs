use super::*;
use rand::prelude::*;
use std::collections::{HashMap, HashSet};

#[test]
fn aabb_simple() {
    let points = [
        Point::new(12, 50),
        Point::new(8, 1),
        Point::new(20, 32),
        Point::new(23, 12),
    ];

    let table = MortonTable::from_iterator(points.iter().cloned().map(|p| (p, 1))).unwrap();
    let [min, max] = table.aabb().unwrap();

    let min = min.as_array();
    let max = max.as_array();

    let [q, r] = min;
    assert!(q <= 8);
    assert!(r <= 1);

    let [q, r] = max;
    assert!(23 <= q);
    assert!(50 <= r);
}

#[test]
fn simple_from_iterator() {
    let mut rng = rand::thread_rng();
    let mut points = [
        Point::new(1, 23),
        Point::new(2, 42),
        Point::new(1 << 15 - 1, 23),
        Point::new(1, 1 << 14 - 2),
        Point::new(rng.gen_range(0, 1 << 15), rng.gen_range(0, 1 << 15)),
        Point::new(rng.gen_range(0, 1 << 15), rng.gen_range(0, 1 << 15)),
        Point::new(rng.gen_range(0, 1 << 15), rng.gen_range(0, 1 << 15)),
        Point::new(rng.gen_range(0, 1 << 15), rng.gen_range(0, 1 << 15)),
        Point::new(rng.gen_range(0, 1 << 15), rng.gen_range(0, 1 << 15)),
        Point::new(rng.gen_range(0, 1 << 15), rng.gen_range(0, 1 << 15)),
        Point::new(rng.gen_range(0, 1 << 15), rng.gen_range(0, 1 << 15)),
        Point::new(rng.gen_range(0, 1 << 15), rng.gen_range(0, 1 << 15)),
        Point::new(rng.gen_range(0, 1 << 15), rng.gen_range(0, 1 << 15)),
        Point::new(rng.gen_range(0, 1 << 15), rng.gen_range(0, 1 << 15)),
        Point::new(rng.gen_range(0, 1 << 15), rng.gen_range(0, 1 << 15)),
        Point::new(rng.gen_range(0, 1 << 15), rng.gen_range(0, 1 << 15)),
    ];
    points.shuffle(&mut rng);
    MortonTable::from_iterator(points.iter().enumerate().map(|(i, p)| (*p, i))).unwrap();
}

#[test]
fn insertions() {
    let mut table = MortonTable::new();

    let r = table.insert(Point::new(16, 32), 123i32);
    assert!(r);
}

#[test]
fn test_range_query_all() {
    let mut rng = rand::thread_rng();

    let mut table = MortonTable::new();

    for i in 0..256 {
        let p = Point {
            q: rng.gen_range(0, 128),
            r: rng.gen_range(0, 128),
        };
        let inserted = table.insert(p, i);
        assert!(inserted);
    }

    let mut res = Vec::new();
    let center = Point::new(64, 64);
    table.find_by_range(
        &center,
        Point::new(0, 0).hex_distance(center) as u32 + 1,
        &mut res,
    );

    assert_eq!(res.len(), 256);
}
#[test]
fn regression_get_by_id_bug1() {
    let points = [
        Point { q: 3, r: 10 },
        Point { q: 5, r: 11 },
        Point { q: 63, r: 5 },
        Point { q: 50, r: 8 },
        Point { q: 63, r: 9 },
        Point { q: 39, r: 25 },
        Point { q: 53, r: 27 },
        Point { q: 14, r: 37 },
        Point { q: 0, r: 46 },
        Point { q: 1, r: 61 },
        Point { q: 30, r: 53 },
        Point { q: 36, r: 39 },
        Point { q: 46, r: 32 },
        Point { q: 58, r: 38 },
        Point { q: 38, r: 59 },
        Point { q: 54, r: 49 },
        Point { q: 82, r: 4 },
        Point { q: 84, r: 14 },
        Point { q: 74, r: 20 },
        Point { q: 77, r: 30 },
        Point { q: 83, r: 23 },
        Point { q: 112, r: 11 },
        Point { q: 99, r: 18 },
        Point { q: 115, r: 29 },
        Point { q: 70, r: 37 },
        Point { q: 64, r: 40 },
        Point { q: 82, r: 32 },
        Point { q: 86, r: 36 },
        Point { q: 70, r: 53 },
        Point { q: 99, r: 35 },
        Point { q: 97, r: 43 },
        Point { q: 108, r: 42 },
        Point { q: 107, r: 62 },
        Point { q: 122, r: 63 },
        Point { q: 17, r: 67 },
        Point { q: 29, r: 66 },
        Point { q: 10, r: 89 },
        Point { q: 31, r: 94 },
        Point { q: 42, r: 75 },
        Point { q: 49, r: 64 },
        Point { q: 62, r: 66 },
        Point { q: 33, r: 90 },
        Point { q: 59, r: 82 },
        Point { q: 60, r: 85 },
        Point { q: 53, r: 93 },
        Point { q: 16, r: 105 },
        Point { q: 55, r: 109 },
        Point { q: 38, r: 121 },
        Point { q: 41, r: 127 },
        Point { q: 73, r: 70 },
        Point { q: 75, r: 70 }, // this is the ficked fucked fuckery
        Point { q: 65, r: 78 },
        Point { q: 76, r: 73 },
        Point { q: 95, r: 65 },
        Point { q: 92, r: 69 },
        Point { q: 87, r: 75 },
        Point { q: 117, r: 69 },
        Point { q: 98, r: 84 },
        Point { q: 120, r: 83 },
        Point { q: 88, r: 97 },
        Point { q: 99, r: 118 },
        Point { q: 110, r: 126 },
        Point { q: 126, r: 122 },
    ];
    let points: Vec<(_, _)> = points
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, p)| (p, i))
        .collect();

    let table = MortonTable::<Point, usize>::from_iterator(points.iter().cloned()).unwrap();

    for p in points {
        let found = table.get_by_id(&p.0);
        let key = MortonKey::new(p.0.q as u16, p.0.r as u16);
        assert_eq!(found, Some(&p.1), "{:?} {:?}", p.0, key);
    }
}

#[test]
fn get_by_id() {
    let mut rng = rand::thread_rng();

    let mut table = MortonTable::<Point, usize>::new();

    let mut points = HashSet::with_capacity(64);

    for _ in 0..64 {
        let p = Point {
            q: rng.gen_range(0, 128),
            r: rng.gen_range(0, 128),
        };
        let i = 1000 * p.q + p.r;
        points.insert((p, i as usize));
    }

    for (p, e) in points.iter() {
        let inserted = table.insert(p.clone(), *e);
        assert!(inserted);
    }

    println!("{:?}\n{:?}", table.skiplist, table.keys);

    for p in points {
        let found = table.get_by_id(&p.0);
        let key = MortonKey::new(p.0.q as u16, p.0.r as u16);
        assert_eq!(found, Some(&p.1), "{:?} {:?}", p.0, key);
    }
}

#[test]
fn morton_key_reconstruction_rand() {
    let mut rng = rand::thread_rng();

    for _ in 0..(1 << 12) {
        let q = rng.gen_range(0, 2000);
        let r = rng.gen_range(0, 2000);

        let morton = MortonKey::new(q, r);

        let res = morton.as_point();

        assert_eq!([q, r], res);
    }
}

#[test]
fn from_iterator_inserts_correctly() {
    let mut rng = rand::thread_rng();

    let len = 1 << 12;
    let mut points = HashMap::with_capacity(len);
    let table = MortonTable::from_iterator((0..len).filter_map(|_| {
        let pos = Point {
            q: rng.gen_range(0, 3900 * 2),
            r: rng.gen_range(0, 3900 * 2),
        };
        if !points.contains_key(&pos) {
            return None;
        }
        let val = rng.next_u32();
        points.insert(pos.clone(), val);
        Some((pos, val))
    }))
    .unwrap();

    for (pos, val) in points {
        let v = *table.get_by_id(&pos).expect("to find the value");
        assert_eq!(val, v);
    }
}

#[test]
fn dedupe_simple() {
    let mut rng = rand::thread_rng();

    let mut table = MortonTable::from_iterator((0..128).flat_map(|_| {
        let pos = Point {
            q: rng.gen_range(0, 3900),
            r: rng.gen_range(0, 3900),
        };
        vec![(pos, 0), (pos, 1), (pos, 3)]
    }))
    .unwrap();
    table.dedupe();

    let mut cnt = 0;

    let positions = table
        .iter()
        .map(|(p, _)| {
            cnt += 1;
            p
        })
        .collect::<HashSet<_>>();
    assert_eq!(positions.len(), 128);
    assert_eq!(cnt, 128);
}
