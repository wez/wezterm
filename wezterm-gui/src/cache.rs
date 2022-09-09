#![allow(dead_code)]
use cache_advisor::CacheAdvisor;
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

/// A cache using a Least-Frequently-Used eviction policy.
/// If K is u64 you should use LfuCacheU64 instead as it has
/// less overhead.
pub struct LfuCache<K, V> {
    hit: &'static str,
    miss: &'static str,
    key_to_id: HashMap<K, u64>,
    map: FnvHashMap<u64, (K, V)>,
    next_id: u64,
    advisor: CacheAdvisor,
    cap: usize,
    cap_func: CapFunc,
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
            key_to_id: HashMap::with_capacity(cap),
            map: FnvHashMap::default(),
            advisor: CacheAdvisor::new(cap, ENTRY_PERCENT),
            next_id: 0,
            cap,
            cap_func,
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn update_config(&mut self, config: &ConfigHandle) {
        let new_cap = (self.cap_func)(config);
        if new_cap != self.cap {
            self.cap = new_cap;
            self.clear();
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.key_to_id.clear();
        self.advisor = CacheAdvisor::new(self.cap, ENTRY_PERCENT);
    }

    fn process_evictions(&mut self, evict: &[(u64, usize)]) {
        for (evict_id, _cost) in evict {
            if let Some((evict_key, _v)) = self.map.remove(&evict_id) {
                self.key_to_id.remove(&evict_key);
            }
        }
    }

    pub fn get<'a, Q: ?Sized>(&'a mut self, k: &Q) -> Option<&'a V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let id = match self.key_to_id.get(k) {
            Some(id) => *id,
            None => {
                metrics::histogram!(self.miss, 1.0);
                return None;
            }
        };

        let evict = self.advisor.accessed(id, 1);
        self.process_evictions(&evict);

        metrics::histogram!(self.hit, 1.);

        self.map.get(&id).map(|(_k, v)| v)
    }

    pub fn put(&mut self, k: K, v: V) -> Option<V> {
        let id = match self.key_to_id.get(&k) {
            Some(id) => *id,
            None => {
                let id = self.next_id;
                self.next_id += 1;
                self.key_to_id.insert(k.clone(), id);
                id
            }
        };

        let evict = self.advisor.accessed(id, 1);
        self.process_evictions(&evict);

        self.map.insert(id, (k, v)).map(|(_k, v)| v)
    }
}

/// A cache using a Least-Frequently-Used eviction policy, where the cache keys
/// are u64
pub struct LfuCacheU64<V> {
    hit: &'static str,
    miss: &'static str,
    map: FnvHashMap<u64, V>,
    advisor: CacheAdvisor,
    cap: usize,
    cap_func: CapFunc,
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
            advisor: CacheAdvisor::new(cap, ENTRY_PERCENT),
            cap,
            cap_func,
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn update_config(&mut self, config: &ConfigHandle) {
        let new_cap = (self.cap_func)(config);
        if new_cap != self.cap {
            self.cap = new_cap;
            self.clear();
        }
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.advisor = CacheAdvisor::new(self.cap, ENTRY_PERCENT);
    }

    fn process_evictions(&mut self, evict: &[(u64, usize)]) {
        for (evict_id, _cost) in evict {
            self.map.remove(&evict_id);
        }
    }

    pub fn get(&mut self, id: &u64) -> Option<&V> {
        if !self.map.contains_key(&id) {
            metrics::histogram!(self.miss, 1.0);
            return None;
        }
        let evict = self.advisor.accessed(*id, 1);
        self.process_evictions(&evict);
        metrics::histogram!(self.hit, 1.);
        self.map.get(&id)
    }

    pub fn get_mut(&mut self, id: &u64) -> Option<&mut V> {
        if !self.map.contains_key(&id) {
            metrics::histogram!(self.miss, 1.0);
            return None;
        }
        let evict = self.advisor.accessed(*id, 1);
        self.process_evictions(&evict);
        metrics::histogram!(self.hit, 1.);
        self.map.get_mut(&id)
    }

    pub fn put(&mut self, id: u64, v: V) -> Option<V> {
        let evict = self.advisor.accessed(id, 1);
        self.process_evictions(&evict);
        self.map.insert(id, v)
    }
}
