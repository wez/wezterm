use crate::config::configuration;
use hdrhistogram::Histogram;
use metrics::{Key, Recorder};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tabout::{tabulate_output, Alignment, Column};

struct Inner {
    histograms: HashMap<Key, Histogram<u64>>,
}

fn pctile_latency(histogram: &Histogram<u64>, p: f64) -> Duration {
    Duration::from_nanos(histogram.value_at_percentile(p))
}

impl Inner {
    fn run(inner: Arc<Mutex<Inner>>) {
        let mut last_print = Instant::now();

        let cols = vec![
            Column {
                name: "STAT".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "p50".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "p75".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "p95".to_string(),
                alignment: Alignment::Left,
            },
        ];

        loop {
            std::thread::sleep(Duration::from_secs(10));
            let seconds = configuration().periodic_stat_logging;
            if seconds == 0 {
                continue;
            }
            if last_print.elapsed() >= Duration::from_secs(seconds) {
                let inner = inner.lock().unwrap();
                let mut data = vec![];
                for (key, histogram) in &inner.histograms {
                    if key.name().ends_with(".size") {
                        let p50 = histogram.value_at_percentile(50.);
                        let p75 = histogram.value_at_percentile(75.);
                        let p95 = histogram.value_at_percentile(95.);
                        data.push(vec![
                            key.to_string(),
                            format!("{:.2?}", p50),
                            format!("{:.2?}", p75),
                            format!("{:.2?}", p95),
                        ]);
                    } else {
                        let p50 = pctile_latency(histogram, 50.);
                        let p75 = pctile_latency(histogram, 75.);
                        let p95 = pctile_latency(histogram, 95.);
                        data.push(vec![
                            key.to_string(),
                            format!("{:.2?}", p50),
                            format!("{:.2?}", p75),
                            format!("{:.2?}", p95),
                        ]);
                    }
                }
                data.sort_by(|a, b| a[0].cmp(&b[0]));
                eprintln!();
                tabulate_output(&cols, &data, &mut std::io::stderr().lock()).ok();
                last_print = Instant::now();
            }
        }
    }
}

pub struct Stats {
    inner: Arc<Mutex<Inner>>,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                histograms: HashMap::new(),
            })),
        }
    }

    pub fn init() -> anyhow::Result<()> {
        let stats = Self::new();
        let inner = Arc::clone(&stats.inner);
        std::thread::spawn(move || Inner::run(inner));
        let rec = Box::new(stats);
        metrics::set_boxed_recorder(rec)
            .map_err(|e| anyhow::anyhow!("Failed to set metrics recorder:{}", e))
    }
}

impl Recorder for Stats {
    fn increment_counter(&self, key: Key, value: u64) {
        log::trace!("counter '{}' -> {}", key, value);
    }

    fn update_gauge(&self, key: Key, value: i64) {
        log::trace!("gauge '{}' -> {}", key, value);
    }

    fn record_histogram(&self, key: Key, value: u64) {
        let mut inner = self.inner.lock().unwrap();
        let histogram = inner
            .histograms
            .entry(key)
            .or_insert_with(|| Histogram::new(2).expect("failed to crate new Histogram"));
        histogram.record(value).ok();
    }
}
