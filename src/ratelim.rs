use ratelimit_meter::algorithms::NonConformance;
use ratelimit_meter::{DirectRateLimiter, LeakyBucket, NegativeMultiDecision};

pub struct RateLimiter {
    lim: DirectRateLimiter<LeakyBucket>,
    capacity_per_second: u32,
}

impl RateLimiter {
    pub fn new(capacity_per_second: u32) -> Self {
        Self {
            lim: DirectRateLimiter::<LeakyBucket>::per_second(
                std::num::NonZeroU32::new(capacity_per_second)
                    .expect("RateLimiter capacity to be non-zero"),
            ),
            capacity_per_second,
        }
    }

    pub fn non_blocking_admittance_check(&mut self, amount: u32) -> bool {
        self.lim.check_n(amount).is_ok()
    }

    pub fn non_blocking_admittance_check_max(&mut self, max_amount0: u32) -> u32 {
        // This limit is specific to rate limit algorithm
        let max_amount = std::cmp::min(max_amount0, self.capacity_per_second);
        match self.lim.check_n(max_amount) {
            Ok(_) => return max_amount,
            Err(NegativeMultiDecision::BatchNonConforming(_, over)) => {
                let duration = over.wait_time_from(std::time::Instant::now());
                // TODO: switch to a rate limiter that supports allocating less than requested
                let overflow_guess = self.capacity_per_second * duration.as_secs() as u32;
                let allowed_guess = max_amount - overflow_guess;
                if self.non_blocking_admittance_check(allowed_guess) {
                    return allowed_guess;
                }
            }
            _ => {}
        }
        return 0;
    }

    pub fn blocking_admittance_check(&mut self, amount: u32) {
        loop {
            match self.lim.check_n(amount) {
                Ok(_) => return,
                Err(NegativeMultiDecision::BatchNonConforming(_, over)) => {
                    let duration = over.wait_time_from(std::time::Instant::now());
                    log::trace!("RateLimiter: sleep for {:?}", duration);
                    std::thread::sleep(duration);
                }
                Err(NegativeMultiDecision::InsufficientCapacity(n)) => {
                    panic!(
                        "Programmer Error: you need to chunk the input \
                         because you're trying to admit {} items at once \
                         and this exceeds the maximum limit of {} per second",
                        n, self.capacity_per_second
                    );
                }
            }
        }
    }
}
