use super::morton_key::MortonKey;
use super::msb_de_bruijn;
use rayon::prelude::*;

const RADIX_MASK_LEN: u8 = 5;
const RADIX_MASK: u32 = (1 << (RADIX_MASK_LEN + 1)) - 1;
const NUM_BUCKETS: usize = RADIX_MASK as usize + 1;

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
    let mut output = vec![Default::default();keys.len()];

    let msb = keys.iter().map(|k| msb_de_bruijn(k.0)).max().unwrap();
    // TODO: optimize using min lsb?
    for k in (0..=msb).step_by(RADIX_MASK_LEN as usize) {
        radix_pass(k as u8, &input[..], &mut output);
        debug_assert_eq!(input.len(), output.len());
        std::mem::swap(&mut input, &mut output);
    }

    // TODO: calculate minimum swaps required
    // TODO: execute swaps

    let mut ks = Vec::with_capacity(keys.len());
    let mut ps = Vec::with_capacity(keys.len());
    let mut vs = Vec::with_capacity(keys.len());

    // input and output were swapped so iter on input
    for (_, i) in input.iter() {
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
    let step = keys.len() / rayon::current_num_threads() + 1;
    // compute the length of each bucket
    let mut counts = keys
        .par_chunks(step)
        .map(|chunk| {
            let mut buckets = [0; NUM_BUCKETS];
            for (key, _) in chunk {
                let bucket = compute_bucket(k, key);
                buckets[bucket] += 1;
            }
            // we'll use the index of the first item as a unique id for each chunk
            (chunk[0].1, buckets)
        })
        .collect::<Vec<_>>();

    // set the output offsets for each bucket
    // this will indicate the 1 after the last index a chunk will occupy
    let mut base = 0;
    for b_ind in 0..NUM_BUCKETS {
        let b_ind = b_ind as usize;
        for buckets in counts.iter_mut() {
            buckets.1[b_ind] += base;
            base = buckets.1[b_ind];
        }
    }

    // write the output
    debug_assert_eq!(keys.len(), out.len());
    let outptr = out.as_mut_ptr();
    let outptr = UnsafeMortonPtr(outptr);

    keys.par_chunks(step).for_each(|chunk| {
        // find the `buckets` of this chunk
        let mut buckets = counts
            .iter()
            .find(|(c, _)| c == &chunk[0].1)
            .map(|(_, b)| b)
            .cloned()
            .expect("bucket counts of chunk");

        for (key, id) in chunk.iter().rev() {
            let bucket = compute_bucket(k, key);
            let index = buckets[bucket] - 1;
            debug_assert!(index < out.len());
            unsafe {
                *outptr.0.add(index) = (*key, *id);
                buckets[bucket] -= 1;
            }
        }
    });
}

#[derive(Clone, Copy)]
struct UnsafeMortonPtr(*mut (MortonKey, usize));
unsafe impl Send for UnsafeMortonPtr {}
unsafe impl Sync for UnsafeMortonPtr {}

fn compute_bucket(k: u8, key: &MortonKey) -> usize {
    let mask = RADIX_MASK << k;
    let ind = key.0 & mask;
    let ind = ind >> k;
    ind as usize
}
