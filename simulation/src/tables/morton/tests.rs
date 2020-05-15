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

    let [x, y] = min;
    assert!(x <= 8);
    assert!(y <= 1);

    let [x, y] = max;
    assert!(23 <= x);
    assert!(50 <= y);
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
            x: rng.gen_range(0, 128),
            y: rng.gen_range(0, 128),
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
        Point { x: 3, y: 10 },
        Point { x: 5, y: 11 },
        Point { x: 63, y: 5 },
        Point { x: 50, y: 8 },
        Point { x: 63, y: 9 },
        Point { x: 39, y: 25 },
        Point { x: 53, y: 27 },
        Point { x: 14, y: 37 },
        Point { x: 0, y: 46 },
        Point { x: 1, y: 61 },
        Point { x: 30, y: 53 },
        Point { x: 36, y: 39 },
        Point { x: 46, y: 32 },
        Point { x: 58, y: 38 },
        Point { x: 38, y: 59 },
        Point { x: 54, y: 49 },
        Point { x: 82, y: 4 },
        Point { x: 84, y: 14 },
        Point { x: 74, y: 20 },
        Point { x: 77, y: 30 },
        Point { x: 83, y: 23 },
        Point { x: 112, y: 11 },
        Point { x: 99, y: 18 },
        Point { x: 115, y: 29 },
        Point { x: 70, y: 37 },
        Point { x: 64, y: 40 },
        Point { x: 82, y: 32 },
        Point { x: 86, y: 36 },
        Point { x: 70, y: 53 },
        Point { x: 99, y: 35 },
        Point { x: 97, y: 43 },
        Point { x: 108, y: 42 },
        Point { x: 107, y: 62 },
        Point { x: 122, y: 63 },
        Point { x: 17, y: 67 },
        Point { x: 29, y: 66 },
        Point { x: 10, y: 89 },
        Point { x: 31, y: 94 },
        Point { x: 42, y: 75 },
        Point { x: 49, y: 64 },
        Point { x: 62, y: 66 },
        Point { x: 33, y: 90 },
        Point { x: 59, y: 82 },
        Point { x: 60, y: 85 },
        Point { x: 53, y: 93 },
        Point { x: 16, y: 105 },
        Point { x: 55, y: 109 },
        Point { x: 38, y: 121 },
        Point { x: 41, y: 127 },
        Point { x: 73, y: 70 },
        Point { x: 75, y: 70 }, // this is the ficked fucked fuckery
        Point { x: 65, y: 78 },
        Point { x: 76, y: 73 },
        Point { x: 95, y: 65 },
        Point { x: 92, y: 69 },
        Point { x: 87, y: 75 },
        Point { x: 117, y: 69 },
        Point { x: 98, y: 84 },
        Point { x: 120, y: 83 },
        Point { x: 88, y: 97 },
        Point { x: 99, y: 118 },
        Point { x: 110, y: 126 },
        Point { x: 126, y: 122 },
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
        let key = MortonKey::new(p.0.x as u16, p.0.y as u16);
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
            x: rng.gen_range(0, 128),
            y: rng.gen_range(0, 128),
        };
        let i = 1000 * p.x + p.y;
        points.insert((p, i as usize));
    }

    for (p, e) in points.iter() {
        let inserted = table.insert(p.clone(), *e);
        assert!(inserted);
    }

    println!("{:?}\n{:?}", table.skiplist, table.keys);

    for p in points {
        let found = table.get_by_id(&p.0);
        let key = MortonKey::new(p.0.x as u16, p.0.y as u16);
        assert_eq!(found, Some(&p.1), "{:?} {:?}", p.0, key);
    }
}

#[test]
fn morton_key_reconstruction_rand() {
    let mut rng = rand::thread_rng();

    for _ in 0..(1 << 12) {
        let x = rng.gen_range(0, 2000);
        let y = rng.gen_range(0, 2000);

        let morton = MortonKey::new(x, y);

        let res = morton.as_point();

        assert_eq!([x, y], res);
    }
}

#[test]
fn from_iterator_inserts_correctly() {
    let mut rng = rand::thread_rng();

    let len = 1 << 12;
    let mut points = HashMap::with_capacity(len);
    let table = MortonTable::from_iterator((0..len).filter_map(|_| {
        let pos = Point {
            x: rng.gen_range(0, 3900 * 2),
            y: rng.gen_range(0, 3900 * 2),
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
            x: rng.gen_range(0, 3900),
            y: rng.gen_range(0, 3900),
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
