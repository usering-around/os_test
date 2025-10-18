use core::time::Duration;

use crate::dev::hpet::Hpet;

/// Time elapsed in femto seconds
pub fn elapsed_fs() -> u128 {
    // will take 2^64 * Hpet::fs_per_tick femto seconds to to overflow.
    // assuming Hpet's tick resolution is in nanoseconds or bigger, it will take at least more than
    // 1e+10 seconds to overflow which is about 317 years
    Hpet::read_main_counter() as u128 * Hpet::fs_per_tick() as u128
}

/// Duration which is small enough (namely, its nanoseconds are smaller than SmallDuration::MAX_NANOS) \
/// Note: smaller than 1e+13 nanoseconds/10000 seconds sufficies
pub struct SmallDuration {
    inner: Duration,
    femto_seconds: u64,
}

impl SmallDuration {
    pub const MAX_NANOS: u64 = u64::MAX / 1_000_000;
    pub fn new(duration: Duration) -> Option<SmallDuration> {
        let fs = duration.as_nanos() * 1_000_000_u128;
        if fs <= u64::MAX as u128 {
            Some(Self {
                inner: duration,
                femto_seconds: fs as u64,
            })
        } else {
            None
        }
    }

    pub fn as_femto_secs(&self) -> u64 {
        self.femto_seconds
    }
    pub fn as_duration(&self) -> &Duration {
        &self.inner
    }
}

/// Start Hpet::timer(0) to throw an interrupt after duration.
/// This can be prone to a race condition if duration so small that setting the timer will already make the Hpet's
/// main counter pass it.
pub fn start_timer(duration: SmallDuration) {
    // todo: needs synchornization
    unsafe {
        let timer = Hpet::timer(0);
        let ticks = duration.as_femto_secs() / Hpet::fs_per_tick();
        timer.set_counter_raw(Hpet::read_main_counter() + ticks);
    }
}

/// Sleep by polling on time::elapsed_fs
pub fn poll_sleep(duration: Duration) {
    let now = elapsed_fs();
    let nanos = duration.as_nanos();
    loop {
        let elapsed_time_fs = elapsed_fs() - now;
        let elapsed_time_ns = elapsed_time_fs / 1_000_000;
        if elapsed_time_ns > nanos {
            break;
        }
    }
}
