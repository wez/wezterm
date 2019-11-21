use ratelimit_meter::algorithms::NonConformance;
use ratelimit_meter::{DirectRateLimiter, LeakyBucket, NegativeMultiDecision};
use std::time::{Duration, Instant};

pub struct RateLimiter {
    lim: DirectRateLimiter<LeakyBucket>,
}

impl RateLimiter {
    pub fn new(capacity_per_second: u32) -> Self {
        Self {
            lim: DirectRateLimiter::<LeakyBucket>::per_second(
                std::num::NonZeroU32::new(capacity_per_second)
                    .expect("RateLimiter capacity to be non-zero"),
            ),
        }
    }

    pub fn non_blocking_admittance_check(&mut self, amount: u32) -> bool {
        self.lim.check_n(amount).is_ok()
    }

    /// Attempt to admit up to `amount` number of items.
    /// On success, returns the amount that were actually admitted,
    /// which may be less than the requested amount.
    /// If no items can be admitted immediately, returns a duration
    /// of time after which the caller should retry to admit.
    pub fn admit_check(&mut self, mut amount: u32) -> Result<u32, Duration> {
        loop {
            match self.lim.check_n(amount) {
                Ok(_) => return Ok(amount),
                Err(NegativeMultiDecision::BatchNonConforming(_, over)) if amount == 1 => {
                    return Err(over.wait_time_from(Instant::now()));
                }
                _ => {}
            };

            // try again with half the size.
            // This isn't a perfectly efficient approach, especially
            // with a very large input buffer size, but it is reasonable;
            // we use a 32k buffer which means that in the worst case
            // (where the buffer is 100% full), we'll take ~15 iterations
            // to reach a decision of a single byte or a sleep delay.
            amount = amount / 2;
        }
    }
}
