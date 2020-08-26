#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod sse {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;
    use std::mem;

    pub const SKIP_LEN: usize = 4;

    #[derive(Debug, Clone)]
    pub struct SkipList(pub [__m128i; SKIP_LEN]);

    impl Default for SkipList {
        fn default() -> Self {
            unsafe {
                Self([
                    _mm_set_epi32(0xefff, 0xefff, 0xefff, 0xefff),
                    _mm_set_epi32(0xefff, 0xefff, 0xefff, 0xefff),
                    _mm_set_epi32(0xefff, 0xefff, 0xefff, 0xefff),
                    _mm_set_epi32(0xefff, 0xefff, 0xefff, 0xefff),
                ])
            }
        }
    }

    impl SkipList {
        pub fn set(&mut self, i: usize, val: i32) {
            unsafe {
                let ind = i / 4;
                let vals: &mut [i32; 4] = mem::transmute(&mut self.0[ind]);
                vals[i - ind] = val;
            }
        }
    }
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
mod normal {
    pub const SKIP_LEN: usize = 16;

    #[derive(Debug, Clone)]
    pub struct SkipList(pub [i32; SKIP_LEN]);
    impl Default for SkipList {
        fn default() -> Self {
            Self([
                0xefff, 0xefff, 0xefff, 0xefff, 0xefff, 0xefff, 0xefff, 0xefff, 0xefff, 0xefff,
                0xefff, 0xefff, 0xefff, 0xefff, 0xefff, 0xefff,
            ])
        }
    }
    impl SkipList {
        pub fn set(&mut self, i: usize, val: i32) {
            self.0[i] = val;
        }
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use self::sse::*;
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
pub use normal::*;
