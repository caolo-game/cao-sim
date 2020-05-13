use super::morton_key::MortonKey;
use super::msb_de_bruijn;
use rayon::prelude::*;

const RADIX_MASK_LEN: u8 = 2;
const RADIX_MASK: u32 = (1 << (RADIX_MASK_LEN + 1)) - 1;

// TODO: create a Sorter struct that holds the temp buffer, so MortonTable may hold one and only
// allocate these once per lifetime, should speed up rebuilds

pub fn sort<Pos: Clone, Row: Clone>(
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

    let mut input: Vec<(MortonKey, usize)> = keys
        .iter()
        .cloned()
        .enumerate()
        .map(|(k, v)| (v, k))
        .collect();
    let mut output = Vec::with_capacity(keys.len());

    let mut inpdx = 0;

    let mut buckets = vec![Vec::with_capacity(keys.len() / 2); RADIX_MASK as usize + 1];

    let msb = keys.iter().map(|k| msb_de_bruijn(k.0)).max().unwrap();

    for k in (0..=msb).step_by(RADIX_MASK_LEN as usize) {
        if inpdx == 0 {
            radix_round(k as u8, &input[..], &mut output, &mut buckets[..]);
        } else {
            radix_round(k as u8, &output[..], &mut input, &mut buckets[..]);
        }
        inpdx = 1 - inpdx;
    }

    // TODO: calculate minimum swaps required
    // TODO: execute swaps

    let mut ks = Vec::with_capacity(keys.len());
    let mut ps = Vec::with_capacity(keys.len());
    let mut vs = Vec::with_capacity(keys.len());

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

fn radix_round(
    k: u8,
    keys: &[(MortonKey, usize)], // key, index pairs
    out: &mut Vec<(MortonKey, usize)>,
    // reusable buffers to save on allocation
    buckets: &mut [Vec<(MortonKey, usize)>],
) {
    buckets.par_iter_mut().for_each(|b| {
        b.clear();
    });
    radix_filter(k, keys, buckets);

    // concat
    out.clear();
    for b in buckets.iter() {
        out.extend_from_slice(&b);
    }
    debug_assert_eq!(out.len(), keys.len());
}

fn radix_filter(k: u8, keys: &[(MortonKey, usize)], buckets: &mut [Vec<(MortonKey, usize)>]) {
    let mask: u32 = RADIX_MASK << k;

    buckets.par_iter_mut().enumerate().for_each(|(ind, b)| {
        for key in keys.iter() {
            let i = (key.0).0 & mask;
            let i = i >> k;
            let i = i as usize;
            if i == ind {
                b.push(*key);
            }
        }
    });
}
