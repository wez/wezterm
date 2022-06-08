use config::{configuration, ConfigHandle};
use governor::clock::{Clock, DefaultClock};
use governor::{NegativeMultiDecision, Quota, RateLimiter as Limiter};
use std::num::NonZeroU32;
use std::time::Duration;

pub struct RateLimiter {
    lim: Limiter<governor::state::direct::NotKeyed, governor::state::InMemoryState, DefaultClock>,
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
            lim: Limiter::direct(Quota::per_second(
                NonZeroU32::new(capacity_per_second).expect("RateLimiter capacity to be non-zero"),
            )),
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
                self.lim = Limiter::direct(Quota::per_second(
                    NonZeroU32::new(value).expect("RateLimiter capacity to be non-zero"),
                ));
                self.capacity_per_second = value;
            }
            self.generation = generation;
        }
    }

    #[allow(dead_code)]
    pub fn non_blocking_admittance_check(&mut self, amount: u32) -> bool {
        self.check_config_reload();
        self.lim
            .check_n(NonZeroU32::new(amount).expect("amount to be non-zero"))
            .is_ok()
    }

    /// Attempt to admit up to `amount` number of items.
    /// On success, returns the amount that were actually admitted,
    /// which may be less than the requested amount.
    /// If no items can be admitted immediately, returns a duration
    /// of time after which the caller should retry to admit.
    pub fn admit_check(&mut self, mut amount: u32) -> Result<u32, Duration> {
        self.check_config_reload();
        loop {
            let non_zero_amount = match NonZeroU32::new(amount) {
                Some(n) => n,
                None => return Ok(0),
            };
            match self.lim.check_n(non_zero_amount) {
                Ok(_) => return Ok(amount),
                Err(NegativeMultiDecision::BatchNonConforming(_, over)) if amount == 1 => {
                    return Err(over.wait_time_from(DefaultClock::default().now()));
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
