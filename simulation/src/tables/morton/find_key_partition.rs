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
pub fn find_key_partition(skiplist: &SkipList, key: MortonKey) -> usize {
    unsafe { find_key_partition_sse2(&skiplist, key) }
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
#[inline(always)]
pub fn find_key_partition(skiplist: &SkipList, key: MortonKey) -> usize {
    let key = key.0 as i32;
    for (i, skip) in skiplist.0.iter().enumerate() {
        if skip > &key {
            return i;
        }
    }
    SKIP_LEN
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline]
unsafe fn find_key_partition_sse2(skiplist: &SkipList, key: MortonKey) -> usize {
    const PARTITIONS: usize = 4; // SKIP_LEN / 4;

    let key = key.0 as i32;
    let keys4 = _mm_set_epi32(key, key, key, key);

    // set every 32 bits to 0xFFFF if key > skip else sets it to 0x0000
    let results = [
        _mm_cmpgt_epi32(keys4, skiplist.0[0]),
        _mm_cmpgt_epi32(keys4, skiplist.0[1]),
        _mm_cmpgt_epi32(keys4, skiplist.0[2]),
        _mm_cmpgt_epi32(keys4, skiplist.0[3]),
    ];

    // create a mask from the most significant bit of each 8bit element
    let masks = [
        _mm_movemask_epi8(results[0]),
        _mm_movemask_epi8(results[1]),
        _mm_movemask_epi8(results[2]),
        _mm_movemask_epi8(results[3]),
    ];

    let mut index = 0;
    for i in 0..PARTITIONS {
        // count the number of bits set to 1
        index += _popcnt32(masks[i]);
    }

    // because the mask was created from 8 bit wide items every key in skip list is counted
    // 4 times.
    index as usize / 4
}
