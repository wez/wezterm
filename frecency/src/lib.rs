use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Frecency tracks stats around when an item was accessed,
/// and provides a score that is a combination of frequency
/// and recency that is useful when presenting the user
/// with a list of previously access items.
#[serde_with::serde_as]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Frecency {
    /// The frecency score decays to half its value when
    /// half_life has elapsed since its previous access.
    #[serde_as(as = "serde_with::DurationSeconds<i64>")]
    half_life: Duration,
    #[serde_as(as = "serde_with::TimestampSeconds<i64>")]
    last_accessed: DateTime<Utc>,
    frecency: f64,
    num_accesses: u64,
}

impl Default for Frecency {
    fn default() -> Self {
        Self::new()
    }
}

impl Frecency {
    /// Creates a new Frecency that initially has no accesses
    pub fn new() -> Self {
        Self::new_at_time(Utc::now())
    }

    /// Creates a new Frecency that initially has no accesses.
    /// `now` is the current time, if you happen to already know it.
    pub fn new_at_time(now: DateTime<Utc>) -> Self {
        Self {
            half_life: Duration::days(3),
            frecency: 0.0,
            last_accessed: now,
            num_accesses: 0,
        }
    }

    /// Record an access; updates internal stats accordingly
    pub fn register_access(&mut self) {
        self.register_access_at_time(Utc::now());
    }

    /// Record an access at a given time; updates internal stats accordingly
    pub fn register_access_at_time(&mut self, now: DateTime<Utc>) {
        let prior = self.score_at_time(now);
        self.last_accessed = now;
        self.set_frecency_at_time(1.0 + prior, now);
        self.num_accesses += 1;
    }

    /// Returns the number of accesses
    pub fn num_accesses(&self) -> u64 {
        self.num_accesses
    }

    /// Returns the time when the item was last accessed
    pub fn last_accessed(&self) -> &DateTime<Utc> {
        &self.last_accessed
    }

    /// Compute the frecency score
    pub fn score(&self) -> f64 {
        self.score_at_time(Utc::now())
    }

    /// Compute the frecency score at a particular time
    pub fn score_at_time(&self, now: DateTime<Utc>) -> f64 {
        let elapsed = duration_secs_f64(now - self.last_accessed);
        self.frecency / 2.0_f64.powf(elapsed / duration_secs_f64(self.half_life))
    }

    fn set_frecency_at_time(&mut self, value: f64, now: DateTime<Utc>) {
        let elapsed = duration_secs_f64(now - self.last_accessed);
        self.frecency = value * 2.0_f64.powf(elapsed / duration_secs_f64(self.half_life));
    }
}

fn duration_secs_f64(dur: Duration) -> f64 {
    dur.num_milliseconds() as f64 / 1000.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        a == b || (a - b).abs() <= f64::EPSILON
    }

    fn assert_approx_eq(a: f64, b: f64) {
        if !approx_eq(a, b) {
            panic!("expected {a} to be approx. {b}");
        }
    }

    #[test]
    fn it_works() {
        let now = Utc::now();
        let mut f = Frecency::new_at_time(now);
        assert_eq!(f.score_at_time(now), 0.);
        f.register_access_at_time(now);
        assert_eq!(f.score_at_time(now), 1.0);

        assert_approx_eq(f.score_at_time(now + Duration::days(1)), 0.7937005259840997);

        // After 3 days (the half life), we expect the frecency to decay to half
        assert_approx_eq(f.score_at_time(now + Duration::days(3)), 0.5);

        // An access adds 1 to the score
        f.register_access_at_time(now + Duration::days(3));
        assert_approx_eq(f.score_at_time(now + Duration::days(3)), 1.5);

        assert_approx_eq(f.score_at_time(now + Duration::days(30)), 0.0029296875);
        assert_approx_eq(
            f.score_at_time(now + Duration::days(300)),
            0.0000000000000000000000000000023665827156630354,
        );
        assert_eq!(f.num_accesses(), 2);
    }

    #[test]
    fn serialize() {
        use chrono::TimeZone;
        let now = Utc.with_ymd_and_hms(2022, 08, 31, 22, 16, 0).unwrap();
        let f = Frecency::new_at_time(now);
        assert_eq!(serde_json::to_string(&f).unwrap(), "{\"half_life\":259200,\"last_accessed\":1661984160,\"frecency\":0.0,\"num_accesses\":0}");
    }
}
