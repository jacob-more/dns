use std::{hint::spin_loop, num::NonZeroU8, ops::{Add, Div}, sync::atomic::Ordering};

use atomic::Atomic;
use bytemuck::{NoUninit, Pod, Zeroable};


#[repr(C)]
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
pub struct RollingAverage {
    total: u32,
    count: u8,
    // Padding out to 8 bytes is required for bytemuck Zeroable and Pod.
    pad1: u8,
    pad2: u16,
}

impl RollingAverage {
    pub fn new() -> Self {
        Self {
            total: 0,
            count: 0,
            pad1: 0,
            pad2: 0,
        }
    }

    pub fn put_next(mut self, value: u32, max_count: NonZeroU8) -> Self {
        if self.count < max_count.into() {
            if let Some(total) = self.total.checked_add(value) {
                self.total = total;
                self.count += 1;
                return self;
            }
        }

        // Don't need to use a check_div since max_count is guaranteed to be non-zero, so if the
        // count were zero, the first case would have run instead.
        let average = u64::from(self.total.div(u32::from(self.count)));
        let value = u64::from(value);
        // Since we up-casted from u32, there is no way for the addition to fail. No need to use a
        // checked add.
        let mut total = u64::from(self.total).add(value);

        // If we have overshot the maximum count, then subtract the average one an extra time.
        if self.count > max_count.into() {
            total = total.saturating_sub(average);
            // self.count will never drop below 1 because max_count is non-zero. If max_count were 0
            // or 1, this conditional wouldn't be run.
            self.count -= 1;
        }

        match u32::try_from(total.saturating_sub(average)) {
            Ok(total) => {
                self.total = total;
                return self;
            },
            Err(_) => {
                self.total = u32::MAX;
                return self;
            },
        }
    }

    pub fn current_average(&self) -> f64 {
        f64::from(self.total).div(f64::from(self.count))
    }
}

/// Similar to `Atomic::fetch_update()` except...
/// 1. it returns the updated value, not the previous value.
/// 2. the input function returns `T`, not `Option<T>`.
/// 3. the return value is never an `Err(T)`.
/// This allows it to work better when updating `RollingAverage`.
pub fn fetch_update<T, F>(atomic: &Atomic<T>, success: Ordering, failure: Ordering, f: F) -> T
where
    T: NoUninit + Clone,
    F: Fn(T) -> T
{
    let mut prev = atomic.load(Ordering::Relaxed);
    loop {
        let next = f(prev);
        match atomic.compare_exchange_weak(prev, next.clone(), success, failure) {
            Ok(_) => {
                return next
            },
            Err(next_prev) => {
                prev = next_prev;
                spin_loop();
            },
        }
    }
}
