use config::configuration;
use config::lua::get_or_create_sub_module;
use config::lua::mlua::Lua;
use hdrhistogram::Histogram;
use metrics::{GaugeValue, Key, Recorder, Unit};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tabout::{tabulate_output, Alignment, Column};

static ENABLE_STAT_PRINT: AtomicBool = AtomicBool::new(true);
lazy_static::lazy_static! {
    static ref INNER: Arc<Mutex<Inner>> = make_inner();
}

struct Throughput {
    hist: Histogram<u64>,
    last: Option<Instant>,
    count: u64,
}

impl Throughput {
    fn new() -> Self {
        Self {
            hist: Histogram::new(2).expect("failed to create histogram"),
            last: None,
            count: 0,
        }
    }

    fn add(&mut self, value: u64) {
        if let Some(ref last) = self.last {
            let elapsed = last.elapsed();
            if elapsed > Duration::from_secs(1) {
                self.hist.record(self.count).ok();
                self.count = 0;
                self.last = Some(Instant::now());
            }
        } else {
            // Start a new window
            self.last = Some(Instant::now());
        };
        self.count += value;
    }

    fn current(&mut self) -> u64 {
        if let Some(ref last) = self.last {
            let elapsed = last.elapsed();
            if elapsed > Duration::from_secs(1) {
                self.hist.record(self.count).ok();
                self.count = 0;
                self.last = Some(Instant::now());
            }
        }
        self.count
    }
}

fn pctile_latency(histogram: &Histogram<u64>, p: f64) -> Duration {
    Duration::from_nanos(histogram.value_at_percentile(p))
}

struct Inner {
    histograms: HashMap<Key, Histogram<u64>>,
    throughput: HashMap<Key, Throughput>,
    counters: HashMap<Key, u64>,
}

impl Inner {
    fn run(inner: Arc<Mutex<Inner>>) {
        let mut last_print = Instant::now();

        let rate_cols = vec![
            Column {
                name: "STAT".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "current".to_string(),
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
        let count_cols = vec![
            Column {
                name: "STAT".to_string(),
                alignment: Alignment::Left,
            },
            Column {
                name: "COUNT".to_string(),
                alignment: Alignment::Left,
            },
        ];

        loop {
            std::thread::sleep(Duration::from_secs(1));

            if !ENABLE_STAT_PRINT.load(Ordering::Acquire) {
                break;
            }

            let seconds = configuration().periodic_stat_logging;
            if seconds == 0 {
                continue;
            }
            if last_print.elapsed() >= Duration::from_secs(seconds) {
                let mut data = vec![];

                let mut inner = inner.lock().unwrap();
                for (key, tput) in &mut inner.throughput {
                    let current = tput.current();
                    let p50 = tput.hist.value_at_percentile(50.);
                    let p75 = tput.hist.value_at_percentile(75.);
                    let p95 = tput.hist.value_at_percentile(95.);
                    data.push(vec![
                        key.to_string(),
                        format!("{:.2?}", current),
                        format!("{:.2?}", p50),
                        format!("{:.2?}", p75),
                        format!("{:.2?}", p95),
                    ]);
                }
                data.sort_by(|a, b| a[0].cmp(&b[0]));
                eprintln!();
                tabulate_output(&rate_cols, &data, &mut std::io::stderr().lock()).ok();

                data.clear();
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

                data.clear();
                for (key, count) in &inner.counters {
                    data.push(vec![key.to_string(), count.to_string()]);
                }
                data.sort_by(|a, b| a[0].cmp(&b[0]));
                eprintln!();
                tabulate_output(&count_cols, &data, &mut std::io::stderr().lock()).ok();

                last_print = Instant::now();
            }
        }
    }
}

fn make_inner() -> Arc<Mutex<Inner>> {
    Arc::new(Mutex::new(Inner {
        histograms: HashMap::new(),
        throughput: HashMap::new(),
        counters: HashMap::new(),
    }))
}

pub struct Stats {
    inner: Arc<Mutex<Inner>>,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            inner: Arc::clone(&INNER),
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
    fn register_counter(
        &self,
        _key: &Key,
        _unit: Option<Unit>,
        _description: Option<&'static str>,
    ) {
    }

    fn register_gauge(&self, _key: &Key, _unit: Option<Unit>, _description: Option<&'static str>) {}

    fn register_histogram(
        &self,
        _key: &Key,
        _unit: Option<Unit>,
        _description: Option<&'static str>,
    ) {
    }

    fn increment_counter(&self, key: &Key, value: u64) {
        let mut inner = self.inner.lock().unwrap();
        let counter = inner.counters.entry(key.clone()).or_insert_with(|| 0);
        *counter = *counter + value;
    }

    fn update_gauge(&self, key: &Key, value: GaugeValue) {
        log::trace!("gauge '{}' -> {:?}", key, value);
    }

    fn record_histogram(&self, key: &Key, value: f64) {
        let mut inner = self.inner.lock().unwrap();
        if key.name().ends_with(".rate") {
            let tput = inner
                .throughput
                .entry(key.clone())
                .or_insert_with(|| Throughput::new());
            tput.add(value as u64);
        } else {
            let value = if key.name().ends_with(".size") {
                value
            } else {
                // Assume seconds; convert to nanoseconds
                value * 1_000_000_000.0
            };
            let histogram = inner
                .histograms
                .entry(key.clone())
                .or_insert_with(|| Histogram::new(2).expect("failed to crate new Histogram"));
            histogram.record(value as u64).ok();
        }
    }
}

pub fn register(lua: &Lua) -> anyhow::Result<()> {
    let metrics_mod = get_or_create_sub_module(lua, "metrics")?;
    metrics_mod.set(
        "get_counters",
        lua.create_function(|_, _: ()| {
            let inner = INNER.lock().unwrap();
            let counters: HashMap<String, u64> = inner
                .counters
                .iter()
                .map(|(k, &v)| (k.name().to_string(), v))
                .collect();
            Ok(counters)
        })?,
    )?;
    metrics_mod.set(
        "get_throughput",
        lua.create_function(|_, _: ()| {
            let mut inner = INNER.lock().unwrap();
            let counters: HashMap<String, HashMap<String, u64>> = inner
                .throughput
                .iter_mut()
                .map(|(k, tput)| {
                    let mut res = HashMap::new();
                    res.insert("current".to_string(), tput.current());
                    res.insert("p50".to_string(), tput.hist.value_at_percentile(50.));
                    res.insert("p75".to_string(), tput.hist.value_at_percentile(75.));
                    res.insert("p95".to_string(), tput.hist.value_at_percentile(95.));
                    (k.name().to_string(), res)
                })
                .collect();
            Ok(counters)
        })?,
    )?;
    metrics_mod.set(
        "get_sizes",
        lua.create_function(|_, _: ()| {
            let mut inner = INNER.lock().unwrap();
            let counters: HashMap<String, HashMap<String, u64>> = inner
                .histograms
                .iter_mut()
                .filter_map(|(key, hist)| {
                    if key.name().ends_with(".size") {
                        let mut res = HashMap::new();
                        res.insert("p50".to_string(), hist.value_at_percentile(50.));
                        res.insert("p75".to_string(), hist.value_at_percentile(75.));
                        res.insert("p95".to_string(), hist.value_at_percentile(95.));
                        Some((key.name().to_string(), res))
                    } else {
                        None
                    }
                })
                .collect();
            Ok(counters)
        })?,
    )?;
    metrics_mod.set(
        "get_latency",
        lua.create_function(|_, _: ()| {
            let mut inner = INNER.lock().unwrap();
            let counters: HashMap<String, HashMap<String, String>> = inner
                .histograms
                .iter_mut()
                .filter_map(|(key, hist)| {
                    if !key.name().ends_with(".size") {
                        let mut res = HashMap::new();
                        res.insert(
                            "p50".to_string(),
                            format!("{:?}", pctile_latency(hist, 50.)),
                        );
                        res.insert(
                            "p75".to_string(),
                            format!("{:?}", pctile_latency(hist, 75.)),
                        );
                        res.insert(
                            "p95".to_string(),
                            format!("{:?}", pctile_latency(hist, 95.)),
                        );
                        Some((key.name().to_string(), res))
                    } else {
                        None
                    }
                })
                .collect();
            Ok(counters)
        })?,
    )?;
    Ok(())
}
