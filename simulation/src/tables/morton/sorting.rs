//! Sort an array of elements by their `MortonKey`
//!
use super::morton_key::MortonKey;
use std::mem::size_of;

const RADIX_MASK_LEN: usize = 8; // how many bits are considered at a time
const RADIX_MASK: u32 = (1 << (RADIX_MASK_LEN + 1)) - 1;
const NUM_BUCKETS: usize = RADIX_MASK as usize + 1;

pub fn sort<T: Send + Clone>(keys: &mut [MortonKey], values: &mut [T]) {
    debug_assert!(
        keys.len() == values.len(),
        "{} {}",
        keys.len(),
        values.len()
    );
    sort_radix(keys, values);
}

/// The first bit set to 1
#[inline]

fn sort_radix<T: Clone>(keys: &mut [MortonKey], values: &mut [T]) {
    debug_assert!(
        keys.len() == values.len(),
        "{} {}",
        keys.len(),
        values.len()
    );
    if keys.len() < 2 {
        return;
    }

    let mut buffa: Vec<_> = keys.iter().cloned().enumerate().collect();
    let mut buffb = vec![Default::default(); keys.len()];
    let mut buffind = 0;

    for k in (0..=size_of::<MortonKey>() * 8 - RADIX_MASK_LEN).step_by(RADIX_MASK_LEN) {
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
    for bucket in buckets.iter_mut() {
        *bucket += base;
        base = *bucket;
    }

    // write the output
    //
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
    let key = key >> k;
    let ind = key & RADIX_MASK;
    ind as usize
}
