use crate::config::{configuration, ConfigHandle};
use ratelimit_meter::algorithms::NonConformance;
use ratelimit_meter::{DirectRateLimiter, LeakyBucket, NegativeMultiDecision};
use std::time::{Duration, Instant};

pub struct RateLimiter {
    lim: DirectRateLimiter<LeakyBucket>,
    get_limit_value: Box<dyn Fn(&ConfigHandle) -> u32 + 'static + Send>,
    generation: usize,
    capacity_per_second: u32,
}

impl RateLimiter {
    /// Construct a new rate limiter.
    /// `get_limit_value` is a function that will extract a limit
    /// from a config handle; the limit will be automatically adjusted
    /// as the config changes.
    /// This will effectively reset the counter if the limit value in
    /// the new generation of config is different to the prior value.
    pub fn new<F: Fn(&ConfigHandle) -> u32 + 'static + Send>(get_limit_value: F) -> Self {
        let config = configuration();
        let generation = config.generation();
        let get_limit_value = Box::new(get_limit_value);
        let capacity_per_second = get_limit_value(&config);
        Self {
            lim: DirectRateLimiter::<LeakyBucket>::per_second(
                std::num::NonZeroU32::new(capacity_per_second)
                    .expect("RateLimiter capacity to be non-zero"),
            ),
            get_limit_value,
            generation,
            capacity_per_second,
        }
    }

    fn check_config_reload(&mut self) {
        let config = configuration();
        let generation = config.generation();
        if generation != self.generation {
            let value = (self.get_limit_value)(&config);
            if value != self.capacity_per_second {
                self.lim = DirectRateLimiter::<LeakyBucket>::per_second(
                    std::num::NonZeroU32::new(value).expect("RateLimiter capacity to be non-zero"),
                );
                self.capacity_per_second = value;
            }
            self.generation = generation;
        }
    }

    pub fn non_blocking_admittance_check(&mut self, amount: u32) -> bool {
        self.check_config_reload();
        self.lim.check_n(amount).is_ok()
    }

    /// Attempt to admit up to `amount` number of items.
    /// On success, returns the amount that were actually admitted,
    /// which may be less than the requested amount.
    /// If no items can be admitted immediately, returns a duration
    /// of time after which the caller should retry to admit.
    pub fn admit_check(&mut self, mut amount: u32) -> Result<u32, Duration> {
        self.check_config_reload();
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
            amount /= 2;
        }
    }
}
