#![allow(dead_code)]
use config::ConfigHandle;
use fnv::FnvHashMap;
use std::borrow::Borrow;
use std::cmp::Eq;
use std::collections::HashMap;
use std::hash::Hash;

/// Use ENTRY_PERCENT of the capacity as the "temporary cache"; entries
/// are evicted from the temporary cache before the main cache.
/// Frequently used items are promoted from temporary to main cache.
const ENTRY_PERCENT: u8 = 20;

pub type CapFunc = fn(&ConfigHandle) -> usize;

struct ValueWithFreq<V> {
    value: V,
    freq: u16,
}

impl<'a, V: 'a> ValueWithFreq<V> {
    /// A very basic LFU algorithm.
    /// If we have a known latest key, just return it.
    /// Otherwise, find the key with the lowest freq by simply
    /// iterating the entire cache.
    /// For large cache sizes, this isn't great.
    pub fn lfu<K: Clone + 'a>(
        latest: &mut Option<K>,
        iter: impl Iterator<Item = (&'a K, &'a ValueWithFreq<V>)>,
    ) -> Option<K> {
        if let Some(key) = latest.take() {
            return Some(key);
        }
        let mut lfu = None;
        for (k, ValueWithFreq { freq, .. }) in iter {
            if let Some((other_key, other_freq)) = lfu.take() {
                if freq < other_freq {
                    lfu.replace((k, freq));
                } else {
                    lfu.replace((other_key, other_freq));
                }
            } else {
                lfu.replace((k, freq));
            }
        }

        lfu.map(|(k, _)| k.clone())
    }
}

/// A cache using a Least-Frequently-Used eviction policy.
/// If K is u64 you should use LfuCacheU64 instead as it has
/// less overhead.
pub struct LfuCache<K, V> {
    hit: &'static str,
    miss: &'static str,
    map: HashMap<K, ValueWithFreq<V>>,
    cap: usize,
    cap_func: CapFunc,
    latest: Option<K>,
}

impl<K: Hash + Eq + Clone, V> LfuCache<K, V> {
    pub fn new(
        hit: &'static str,
        miss: &'static str,
        cap_func: CapFunc,
        config: &ConfigHandle,
    ) -> Self {
        let cap = cap_func(config);
        Self {
            hit,
            miss,
            map: HashMap::with_capacity(cap),
            cap,
            cap_func,
            latest: None,
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn update_config(&mut self, config: &ConfigHandle) {
        let new_cap = (self.cap_func)(config);
        if new_cap != self.cap {
            self.cap = new_cap;
            self.map = HashMap::with_capacity(new_cap);
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    pub fn get<'a, Q: ?Sized>(&'a mut self, k: &Q) -> Option<&'a V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        match self.map.get_mut(k) {
            None => {
                metrics::histogram!(self.miss, 1.);
                None
            }
            Some(ValueWithFreq { value, freq }) => {
                metrics::histogram!(self.hit, 1.);
                *freq = freq.saturating_add(1);
                match &self.latest {
                    Some(latest) if latest.borrow() == k => {
                        self.latest.take();
                    }
                    _ => {}
                }
                Some(value)
            }
        }
    }

    pub fn put(&mut self, k: K, v: V) -> Option<V> {
        let prior = self.map.remove(&k);
        if self.map.len() >= self.cap {
            let lfu = ValueWithFreq::lfu(&mut self.latest, self.map.iter());
            if let Some(key) = lfu {
                self.map.remove(&key);
            }
        }
        self.latest.replace(k.clone());
        self.map.insert(k, ValueWithFreq { value: v, freq: 0 });
        prior.map(|ent| ent.value)
    }
}

/// A cache using a Least-Frequently-Used eviction policy, where the cache keys
/// are u64
pub struct LfuCacheU64<V> {
    hit: &'static str,
    miss: &'static str,
    map: FnvHashMap<u64, ValueWithFreq<V>>,
    cap: usize,
    cap_func: CapFunc,
    latest: Option<u64>,
}

impl<V> LfuCacheU64<V> {
    pub fn new(
        hit: &'static str,
        miss: &'static str,
        cap_func: CapFunc,
        config: &ConfigHandle,
    ) -> Self {
        let cap = cap_func(config);
        Self {
            hit,
            miss,
            map: FnvHashMap::default(),
            cap,
            cap_func,
            latest: None,
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn update_config(&mut self, config: &ConfigHandle) {
        let new_cap = (self.cap_func)(config);
        if new_cap != self.cap {
            self.cap = new_cap;
            self.map = FnvHashMap::default();
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
    }

    pub fn get(&mut self, id: &u64) -> Option<&V> {
        match self.map.get_mut(&id) {
            None => {
                metrics::histogram!(self.miss, 1.0);
                None
            }
            Some(ValueWithFreq { value, freq }) => {
                metrics::histogram!(self.hit, 1.);
                *freq = freq.saturating_add(1);
                match &self.latest {
                    Some(latest) if latest == id => {
                        self.latest.take();
                    }
                    _ => {}
                }
                Some(value)
            }
        }
    }

    pub fn get_mut(&mut self, id: &u64) -> Option<&mut V> {
        match self.map.get_mut(&id) {
            None => {
                metrics::histogram!(self.miss, 1.0);
                None
            }
            Some(ValueWithFreq { value, freq }) => {
                metrics::histogram!(self.hit, 1.);
                *freq = freq.saturating_add(1);
                match &self.latest {
                    Some(latest) if latest == id => {
                        self.latest.take();
                    }
                    _ => {}
                }
                Some(value)
            }
        }
    }

    pub fn put(&mut self, id: u64, v: V) -> Option<V> {
        let prior = self.map.remove(&id);
        if self.map.len() >= self.cap {
            let lfu = ValueWithFreq::lfu(&mut self.latest, self.map.iter());
            if let Some(key) = lfu {
                self.map.remove(&key);
            }
        }
        self.latest.replace(id);
        self.map.insert(id, ValueWithFreq { value: v, freq: 0 });
        prior.map(|ent| ent.value)
    }
}
