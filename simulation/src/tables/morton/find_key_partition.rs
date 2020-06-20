//! Find the index of the partition where `key` _might_ reside.
//! This is the index of the first item in the `skiplist` that is greater than the `key`
//!
use super::*;
#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline(always)]
pub fn find_key_partition(skiplist: &[u32; SKIP_LEN], key: MortonKey) -> usize {
    if is_x86_feature_detected!("sse2") {
        unsafe { find_key_partition_sse2(&skiplist, key) }
    } else {
        find_key_partition_serial(&skiplist, key)
    }
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[inline(always)]
pub fn find_key_partition(skiplist: &[u32; SKIP_LEN], key: MortonKey) -> usize {
    find_key_partition_serial(&skiplist, key)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline]
unsafe fn find_key_partition_sse2(skiplist: &[u32; SKIP_LEN], key: MortonKey) -> usize {
    let key = key.0 as i32;
    let keys4 = _mm_set_epi32(key, key, key, key);

    let [s0, s1, s2, s3, s4, s5, s6, s7]: [i32; SKIP_LEN] = std::mem::transmute(*skiplist);
    let skiplist_a: __m128i = _mm_set_epi32(s0, s1, s2, s3);
    let skiplist_b: __m128i = _mm_set_epi32(s4, s5, s6, s7);

    // set every 32 bits to 0xFFFF if key > skip else sets it to 0x0000
    let results_a = _mm_cmpgt_epi32(keys4, skiplist_a);
    let results_b = _mm_cmpgt_epi32(keys4, skiplist_b);

    // create a mask from the most significant bit of each 8bit element
    let mask_a = _mm_movemask_epi8(results_a);
    let mask_b = _mm_movemask_epi8(results_b);

    // count the number of bits set to 1
    let index = _popcnt32(mask_a) + _popcnt32(mask_b);
    // because the mask was created from 8 bit wide items every key in skip list is counted
    // 4 times.
    index as usize / 4
}

#[inline]
fn find_key_partition_serial(skiplist: &[u32; SKIP_LEN], key: MortonKey) -> usize {
    let key = &key.0;
    for (i, skip) in skiplist.iter().enumerate() {
        if skip > key {
            return i;
        }
    }
    SKIP_LEN
}
