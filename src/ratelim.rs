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

    pub fn capacity_per_second(&self) -> usize {
        self.capacity_per_second as usize
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
