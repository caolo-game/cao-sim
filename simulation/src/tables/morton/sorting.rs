use super::morton_key::MortonKey;
use super::msb_de_bruijn;

const RADIX_MASK_LEN: usize = 4;
const RADIX_MASK: u32 = (1 << (RADIX_MASK_LEN + 1)) - 1;
const NUM_BUCKETS: usize = RADIX_MASK as usize + 1;

pub fn sort<Pos: Send + Clone, Row: Send + Clone>(
    keys: &mut [MortonKey],
    positions: &mut [Pos],
    values: &mut [Row],
) {
    debug_assert!(
        keys.len() == positions.len(),
        "{} {}",
        keys.len(),
        positions.len()
    );
    debug_assert!(
        keys.len() == values.len(),
        "{} {}",
        keys.len(),
        values.len()
    );
    if keys.len() < 2050 {
        sort_radix(keys, positions, values);
        return;
    }
    let pivot = sort_partition(keys, positions, values);
    let (klo, khi) = keys.split_at_mut(pivot);
    let (plo, phi) = positions.split_at_mut(pivot);
    let (vlo, vhi) = values.split_at_mut(pivot);
    rayon::join(
        || sort(klo, plo, vlo),
        || sort(&mut khi[1..], &mut phi[1..], &mut vhi[1..]),
    );
}

/// Assumes that all 3 slices are equal in size.
/// Assumes that the slices are not empty
fn sort_partition<Pos, Row>(
    keys: &mut [MortonKey],
    positions: &mut [Pos],
    values: &mut [Row],
) -> usize {
    debug_assert!(!keys.is_empty());

    macro_rules! swap {
        ($i: expr, $j: expr) => {
            keys.swap($i, $j);
            positions.swap($i, $j);
            values.swap($i, $j);
        };
    };

    let len = keys.len();
    let lim = len - 1;

    let (pivot, pivot_ind) = {
        use std::mem::swap;
        // choose the median of the first, middle and last elements as the pivot

        let mut first = 0;
        let mut last = lim;
        let mut median = len / 2;

        if keys[last] < keys[median] {
            swap(&mut median, &mut last);
        }
        if keys[last] < keys[first] {
            swap(&mut last, &mut first);
        }
        if keys[median] < keys[first] {
            swap(&mut median, &mut first);
        }
        (keys[median], median)
    };

    swap!(pivot_ind, lim);

    let mut i = 0; // index of the last item <= pivot
    for j in 0..lim {
        if keys[j] < pivot {
            swap!(i, j);
            i += 1;
        }
    }
    swap!(i, lim);
    i
}

fn sort_radix<Pos: Clone, Row: Clone>(
    keys: &mut [MortonKey],
    positions: &mut [Pos],
    values: &mut [Row],
) {
    debug_assert!(
        keys.len() == positions.len(),
        "{} {}",
        keys.len(),
        positions.len()
    );
    debug_assert!(
        keys.len() == values.len(),
        "{} {}",
        keys.len(),
        values.len()
    );
    if keys.len() < 2 {
        return;
    }

    let mut buffa: Vec<_> = keys
        .iter()
        .cloned()
        .enumerate()
        .map(|(k, v)| (v, k))
        .collect();
    let mut buffb = vec![Default::default(); keys.len()];
    let mut buffind = 0;

    // let mut counts = vec![Default::default();

    let msb = keys.iter().map(|k| msb_de_bruijn(k.0)).max().unwrap();
    // TODO: optimize using min lsb?
    for k in (0..=msb).step_by(RADIX_MASK_LEN as usize) {
        if buffind == 0 {
            radix_pass(k as u8, &buffa[..], &mut buffb);
        } else {
            radix_pass(k as u8, &buffb[..], &mut buffa);
        }
        debug_assert_eq!(buffa.len(), buffb.len());
        buffind = 1 - buffind;
    }

    // TODO: calculate minimum swaps required
    // TODO: execute swaps

    let mut ks = Vec::with_capacity(keys.len());
    let mut ps = Vec::with_capacity(keys.len());
    let mut vs = Vec::with_capacity(keys.len());

    let output = if buffind == 0 { buffa } else { buffb };
    for (_, i) in output.iter() {
        let i = *i;
        ks.push(keys[i]);
        ps.push(positions[i].clone());
        vs.push(values[i].clone());
    }

    ks.swap_with_slice(keys);
    ps.swap_with_slice(positions);
    vs.swap_with_slice(values);
}

fn radix_pass(
    k: u8,
    keys: &[(MortonKey, usize)], // key, index pairs
    out: &mut [(MortonKey, usize)],
) {
    let mut buckets = [0; NUM_BUCKETS];
    // compute the length of each bucket
    keys.iter().for_each(|(key, _)| {
        let bucket = compute_bucket(k, key);
        buckets[bucket] += 1;
    });

    // set the output offsets for each bucket
    // this will indicate the 1 after the last index a chunk will occupy
    let mut base = 0;
    for b_ind in 0..NUM_BUCKETS {
        buckets[b_ind] += base;
        base = buckets[b_ind];
    }

    // write the output
    debug_assert_eq!(keys.len(), out.len());

    keys.iter().rev().for_each(|(key, id)| {
        let bucket = compute_bucket(k, key);
        buckets[bucket] -= 1;
        let index = buckets[bucket];
        debug_assert!(index < out.len());
        out[index] = (*key, *id);
    });
}

#[derive(Clone, Copy)]
struct UnsafePtr<T>(*mut T);
unsafe impl<T> Send for UnsafePtr<T> {}
unsafe impl<T> Sync for UnsafePtr<T> {}

fn compute_bucket(k: u8, key: &MortonKey) -> usize {
    let mask = RADIX_MASK << k;
    let ind = key.0 & mask;
    let ind = ind >> k;
    ind as usize
}
