//! Sort an array of elements by their `MortonKey`
//!
use super::morton_key::MortonKey;
use super::msb_de_bruijn;

const RADIX_MASK_LEN: usize = 8;
const RADIX_MASK: u32 = (1 << (RADIX_MASK_LEN + 1)) - 1;
const NUM_BUCKETS: usize = RADIX_MASK as usize + 1;
const RADIX_MAX_ELEMENTS: usize = 4096;

pub fn sort<T: Send + Clone>(keys: &mut [MortonKey], values: &mut [T]) {
    // Uses a hybrid scheme. If the array is longer than RADIX_MAX_ELEMENTS we use
    // (parallel) quicksort. Once the number of elements is within range we switch to radix sort.
    debug_assert!(
        keys.len() == values.len(),
        "{} {}",
        keys.len(),
        values.len()
    );
    if keys.len() <= RADIX_MAX_ELEMENTS {
        sort_radix(keys, values);
        return;
    }
    let pivot = sort_partition(keys, values);
    let (klo, khi) = keys.split_at_mut(pivot);
    let (vlo, vhi) = values.split_at_mut(pivot);

    #[cfg(not(feature = "disable-parallelism"))]
    rayon::join(|| sort(klo, vlo), || sort(&mut khi[1..], &mut vhi[1..]));
    #[cfg(feature = "disable-parallelism")]
    {
        sort(klo, vlo);
        sort(&mut khi[1..], &mut vhi[1..]);
    }
}

/// Assumes that all 3 slices are equal in size.
/// Assumes that the slices are not empty
fn sort_partition<T>(keys: &mut [MortonKey], values: &mut [T]) -> usize {
    debug_assert!(!keys.is_empty());

    macro_rules! swap {
        ($i: expr, $j: expr) => {
            keys.swap($i, $j);
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

    let mut index = 0; // index of the last item <= pivot
    for j in 0..lim {
        if keys[j] < pivot {
            swap!(index, j);
            index += 1;
        }
    }
    swap!(index, lim);
    index
}

fn sort_radix<T: Clone>(keys: &mut [MortonKey], values: &mut [T]) {
    debug_assert!(
        keys.len() == values.len(),
        "{} {}",
        keys.len(),
        values.len()
    );
    debug_assert!(keys.len() <= RADIX_MAX_ELEMENTS);
    if keys.len() < 2 {
        return;
    }

    let mut buffa: Vec<_> = keys.iter().cloned().enumerate().collect();
    let mut buffb = vec![Default::default(); keys.len()];
    let mut buffind = 0;

    let mut msb = msb_de_bruijn(keys[0].0);
    for MortonKey(k) in &keys[1..] {
        msb = msb.max(msb_de_bruijn(*k));
    }
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

    let mut ks = Vec::with_capacity(keys.len());
    let mut vs = Vec::with_capacity(keys.len());

    let output = if buffind == 0 { buffa } else { buffb };
    for (i, _) in output.into_iter() {
        ks.push(keys[i]);
        vs.push(values[i].clone());
    }

    ks.swap_with_slice(keys);
    vs.swap_with_slice(values);
}

fn radix_pass(
    k: u8,
    keys: &[(usize, MortonKey)], // key, index pairs
    out: &mut [(usize, MortonKey)],
) {
    let mut buckets = [0; NUM_BUCKETS];
    // compute the length of each bucket
    keys.iter().for_each(|(_, key)| {
        let bucket = compute_bucket(k, *key);
        buckets[bucket] += 1;
    });

    // set the output offsets for each bucket
    // this will indicate the 1 after the last index a chunk will occupy
    let mut base = 0;
    for bucket in buckets.iter_mut().take(NUM_BUCKETS) {
        *bucket += base;
        base = *bucket;
    }

    // write the output
    debug_assert_eq!(keys.len(), out.len());

    keys.iter().rev().for_each(|(id, key)| {
        let bucket = compute_bucket(k, *key);
        buckets[bucket] -= 1;
        let index = buckets[bucket];
        debug_assert!(index < out.len());
        out[index] = (*id, *key);
    });
}

#[inline(always)]
fn compute_bucket(k: u8, MortonKey(key): MortonKey) -> usize {
    let mask = RADIX_MASK << k;
    let ind = key & mask;
    let ind = ind >> k;
    ind as usize
}
