use ahash::AHasher;
use config::ConfigHandle;
use intrusive_collections::{
    intrusive_adapter, Bound, KeyAdapter, LinkedList, LinkedListLink, RBTree, RBTreeLink,
};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::cmp::Eq;
use std::fmt::Debug;
use std::hash::{BuildHasher, BuildHasherDefault, Hash, Hasher};
use std::rc::Rc;

struct Entry<K, V> {
    hash_link: LinkedListLink,
    recency_link: LinkedListLink,
    frequency_link: RBTreeLink,
    freq: RefCell<u16>,
    last_tick: RefCell<u32>,
    key: K,
    value: V,
}

intrusive_adapter!(RecencyAdapter<K,V> = Rc<Entry<K,V>>: Entry<K,V> { recency_link: LinkedListLink });
intrusive_adapter!(FrequenceAdapter<K,V> = Rc<Entry<K,V>>: Entry<K,V> { frequency_link: RBTreeLink });
intrusive_adapter!(HashAdapter<K,V> = Rc<Entry<K,V>>: Entry<K,V> { hash_link: LinkedListLink });

/// Key by Entry::freq
impl<'a, K, V> KeyAdapter<'a> for FrequenceAdapter<K, V> {
    type Key = u16;
    fn get_key(&self, entry: &'a Entry<K, V>) -> u16 {
        *entry.freq.borrow()
    }
}

pub type CapFunc = fn(&ConfigHandle) -> usize;

/// A cache using a Least-Frequently-Used eviction policy.
/// If K is u64 you should use LfuCacheU64 instead as it has
/// a more optimal hasher for integer keys.
pub struct LfuCache<K, V, S = BuildHasherDefault<AHasher>> {
    hit: &'static str,
    miss: &'static str,
    cap: usize,
    cap_func: CapFunc,
    hasher: S,

    /// hash buckets for key-based lookup
    buckets: Vec<LinkedList<HashAdapter<K, V>>>,
    /// frequency-keyed rb-tree
    frequency_index: RBTree<FrequenceAdapter<K, V>>,
    /// the back is the least-recently-used whereas the front is the
    /// most-recently-used
    recency_index: LinkedList<RecencyAdapter<K, V>>,
    /// Number of items in the cache
    len: usize,
    /// tracks number of operations that affect the frequency/age of entries
    tick: u32,
}

impl<K: Hash + Eq + Clone + Debug, V, S: Default + BuildHasher> LfuCache<K, V, S> {
    #[cfg(test)]
    fn with_capacity(cap: usize) -> Self {
        let mut buckets = vec![];
        let num_buckets = (cap / 10).next_power_of_two();
        for _ in 0..num_buckets {
            buckets.push(LinkedList::new(HashAdapter::new()));
        }

        let hasher = S::default();

        fn dummy_cap_func(_: &ConfigHandle) -> usize {
            8
        }

        Self {
            hit: "hit",
            miss: "miss",
            cap,
            cap_func: dummy_cap_func,
            buckets,
            frequency_index: RBTree::new(FrequenceAdapter::new()),
            recency_index: LinkedList::new(RecencyAdapter::new()),
            len: 0,
            tick: 0,
            hasher,
        }
    }

    pub fn new(
        hit: &'static str,
        miss: &'static str,
        cap_func: CapFunc,
        config: &ConfigHandle,
    ) -> Self {
        let cap = cap_func(config);
        let mut buckets = vec![];
        let num_buckets = (cap / 10).next_power_of_two();
        for _ in 0..num_buckets {
            buckets.push(LinkedList::new(HashAdapter::new()));
        }

        let hasher = S::default();

        Self {
            hit,
            miss,
            cap,
            cap_func,
            buckets,
            frequency_index: RBTree::new(FrequenceAdapter::new()),
            recency_index: LinkedList::new(RecencyAdapter::new()),
            len: 0,
            tick: 0,
            hasher,
        }
    }

    fn bucket_for_key<Q: Hash>(&self, k: &Q) -> usize {
        let mut hasher = self.hasher.build_hasher();
        k.hash(&mut hasher);
        (hasher.finish() as usize) % self.buckets.len()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Grow the hash buckets in the pursuit of reducing potential
    /// key collisions in any given bucket
    fn grow_hash(&mut self) {
        let num_buckets = self.buckets.len() * 2;
        let mut buckets = vec![];
        for _ in 0..num_buckets {
            buckets.push(LinkedList::new(HashAdapter::new()));
        }
        std::mem::swap(&mut buckets, &mut self.buckets);

        for mut old_bucket in buckets {
            while let Some(entry) = old_bucket.pop_front() {
                let bucket = self.bucket_for_key(&entry.key);
                self.buckets[bucket].push_front(entry);
            }
        }
    }

    pub fn update_config(&mut self, config: &ConfigHandle) {
        let new_cap = (self.cap_func)(config);
        if new_cap != self.cap {
            self.cap = new_cap;
            while self.len > self.cap {
                self.evict_one();
            }
        }
    }

    /// In order to mitigate previously-very-hot entries that are
    /// not currently being accessed from occupying the bulk of the
    /// table, this function finds the least-recently-accessed item
    /// and decreases its frequency value based on the number of ticks
    /// that have occurred since its last use.
    fn decay_least_recent(&mut self) {
        let mut cursor = self.recency_index.back_mut();
        if let Some(entry) = cursor.get() {
            if *entry.freq.borrow() == 0 {
                // No point removing/reinserting in the rbtree if the freq
                // is already 0
                return;
            }

            let delta = ((self.tick - *entry.last_tick.borrow()) / 10) as u16;
            if delta <= 1 {
                // No point removing/reinserting in the rbtree if there is no change
                return;
            }

            // Adjust lfu
            unsafe {
                let lfu_entry = self
                    .frequency_index
                    .cursor_mut_from_ptr(entry)
                    .remove()
                    .unwrap();
                {
                    let mut freq = entry.freq.borrow_mut();
                    *freq = *freq / delta;
                }
                self.frequency_index.insert(lfu_entry);
            }

            // Adjust lru so that we don't immediately revisit this one
            // on the next decay_least_recent() call
            let lru_entry = cursor.remove().unwrap();
            self.recency_index.push_front(lru_entry);
        }
    }

    /// Remove the entry with the smallest frequency value
    fn evict_one(&mut self) {
        self.decay_least_recent();

        let mut cursor = self.frequency_index.lower_bound_mut(Bound::Included(&0));
        if let Some(entry) = cursor.remove() {
            let bucket = self.bucket_for_key(&entry.key);
            unsafe {
                self.buckets
                    .get_mut(bucket)
                    .unwrap()
                    .cursor_mut_from_ptr(&*entry)
                    .remove();
                self.recency_index.cursor_mut_from_ptr(&*entry).remove();
            }
            self.len -= 1;
        }
    }

    pub fn clear(&mut self) {
        self.frequency_index.clear();
        self.recency_index.clear();
        for bucket in &mut self.buckets {
            bucket.clear();
        }
        self.len = 0;
    }

    pub fn get<'a, Q: ?Sized + Debug>(&'a mut self, k: &Q) -> Option<&'a V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let bucket = self.bucket_for_key(&k);
        let mut cursor = self.buckets.get_mut(bucket)?.front_mut();
        while let Some(entry) = cursor.get() {
            if entry.key.borrow() == k {
                // Adjust lru
                unsafe {
                    let lru_entry = self
                        .recency_index
                        .cursor_mut_from_ptr(entry)
                        .remove()
                        .unwrap();
                    self.recency_index.push_front(lru_entry);
                }

                let entry = cursor.into_ref()?;
                metrics::histogram!(self.hit).record(1.);

                self.tick += 1;

                *entry.last_tick.borrow_mut() = self.tick;

                // Adjust lfu
                unsafe {
                    let lfu_entry = self
                        .frequency_index
                        .cursor_mut_from_ptr(entry)
                        .remove()
                        .unwrap();
                    {
                        let mut freq = lfu_entry.freq.borrow_mut();
                        *freq = freq.saturating_add(1);
                    }
                    self.frequency_index.insert(lfu_entry);
                }

                return Some(&entry.value);
            }

            cursor.move_next();
        }
        metrics::histogram!(self.miss).record(1.);
        None
    }

    pub fn put(&mut self, k: K, v: V) {
        let bucket = self.bucket_for_key(&k);

        self.tick += 1;

        // Remove any prior value
        {
            let mut cursor = self
                .buckets
                .get_mut(bucket)
                .expect("valid bucket index")
                .front_mut();
            while let Some(entry) = cursor.get() {
                if entry.key == k {
                    unsafe {
                        self.frequency_index.cursor_mut_from_ptr(entry).remove();
                        self.recency_index.cursor_mut_from_ptr(entry).remove();
                    }
                    cursor.remove();
                    self.len -= 1;
                    break;
                }
                cursor.move_next();
            }
        }

        while self.len >= self.cap {
            self.evict_one();
        }

        let entry = Rc::new(Entry {
            key: k,
            value: v,
            freq: RefCell::new(0),
            recency_link: LinkedListLink::new(),
            frequency_link: RBTreeLink::new(),
            hash_link: LinkedListLink::new(),
            last_tick: RefCell::new(self.tick),
        });
        self.buckets[bucket].push_front(Rc::clone(&entry));
        self.frequency_index.insert(Rc::clone(&entry));
        self.recency_index.push_front(entry);
        self.len += 1;
        if self.buckets.len() < self.cap && self.len > self.buckets.len() / 2 {
            self.grow_hash();
        }
    }
}

/// A cache using a Least-Frequently-Used eviction policy,
/// where the cache keys are u64
pub type LfuCacheU64<V> = LfuCache<u64, V, fnv::FnvBuildHasher>;

#[cfg(test)]
mod test {
    use super::*;

    #[derive(Debug)]
    #[allow(dead_code)]
    struct EntryData<'a, K, V> {
        freq: u16,
        last_tick: u32,
        key: &'a K,
        value: &'a V,
    }

    impl<'a, K, V> EntryData<'a, K, V> {
        fn new(item: &'a Entry<K, V>) -> Self {
            Self {
                freq: *item.freq.borrow(),
                last_tick: *item.last_tick.borrow(),
                key: &item.key,
                value: &item.value,
            }
        }
    }

    fn frequency_order<K, V, S>(cache: &LfuCache<K, V, S>) -> Vec<EntryData<'_, K, V>> {
        let mut entries = vec![];
        for item in cache.frequency_index.iter() {
            entries.push(EntryData::new(item));
        }
        entries
    }

    fn recency_order<K, V, S>(cache: &LfuCache<K, V, S>) -> Vec<EntryData<'_, K, V>> {
        let mut entries = vec![];
        for item in cache.recency_index.iter() {
            entries.push(EntryData::new(item));
        }
        entries
    }

    #[test]
    fn decay() {
        let mut cache = LfuCacheU64::with_capacity(4);
        for i in 0..4 {
            cache.put(i, i);
            for _ in 0..i * 2 {
                cache.get(&i);
            }
        }
        k9::snapshot!(
            frequency_order(&cache),
            "
[
    EntryData {
        freq: 0,
        last_tick: 1,
        key: 0,
        value: 0,
    },
    EntryData {
        freq: 2,
        last_tick: 4,
        key: 1,
        value: 1,
    },
    EntryData {
        freq: 4,
        last_tick: 9,
        key: 2,
        value: 2,
    },
    EntryData {
        freq: 6,
        last_tick: 16,
        key: 3,
        value: 3,
    },
]
"
        );

        cache.get(&1);
        cache.get(&2);
        cache.put(10, 10);

        k9::snapshot!(
            frequency_order(&cache),
            "
[
    EntryData {
        freq: 0,
        last_tick: 19,
        key: 10,
        value: 10,
    },
    EntryData {
        freq: 3,
        last_tick: 17,
        key: 1,
        value: 1,
    },
    EntryData {
        freq: 5,
        last_tick: 18,
        key: 2,
        value: 2,
    },
    EntryData {
        freq: 6,
        last_tick: 16,
        key: 3,
        value: 3,
    },
]
"
        );

        cache.get(&10);
        cache.put(11, 11);
        // bump up freq of 11 so that we can displace 1 on the next put
        cache.get(&11);
        cache.get(&11);
        cache.get(&11);
        cache.get(&11);
        k9::snapshot!(
            frequency_order(&cache),
            "
[
    EntryData {
        freq: 3,
        last_tick: 17,
        key: 1,
        value: 1,
    },
    EntryData {
        freq: 4,
        last_tick: 25,
        key: 11,
        value: 11,
    },
    EntryData {
        freq: 5,
        last_tick: 18,
        key: 2,
        value: 2,
    },
    EntryData {
        freq: 6,
        last_tick: 16,
        key: 3,
        value: 3,
    },
]
"
        );

        cache.put(12, 12);
        k9::snapshot!(
            frequency_order(&cache),
            "
[
    EntryData {
        freq: 0,
        last_tick: 26,
        key: 12,
        value: 12,
    },
    EntryData {
        freq: 4,
        last_tick: 25,
        key: 11,
        value: 11,
    },
    EntryData {
        freq: 5,
        last_tick: 18,
        key: 2,
        value: 2,
    },
    EntryData {
        freq: 6,
        last_tick: 16,
        key: 3,
        value: 3,
    },
]
"
        );

        // Ensure that we're all non-zero
        for _ in 0..5 {
            cache.get(&2);
            cache.get(&11);
            cache.get(&12);
        }

        // and bump up the ticks so that we trigger decay for 3
        for _ in 0..10 {
            cache.get(&11);
        }

        // Note that key: 3 has freq 6 in this snapshot
        k9::snapshot!(
            frequency_order(&cache),
            "
[
    EntryData {
        freq: 5,
        last_tick: 41,
        key: 12,
        value: 12,
    },
    EntryData {
        freq: 6,
        last_tick: 16,
        key: 3,
        value: 3,
    },
    EntryData {
        freq: 10,
        last_tick: 39,
        key: 2,
        value: 2,
    },
    EntryData {
        freq: 19,
        last_tick: 51,
        key: 11,
        value: 11,
    },
]
"
        );

        // trigger an eviction. This will decay key 3's freq
        // and it will be evicted, even though key 12 in
        // the snapshot above had freq 5 when key 3 had freq 6.
        cache.put(42, 42);
        k9::snapshot!(
            frequency_order(&cache),
            "
[
    EntryData {
        freq: 0,
        last_tick: 52,
        key: 42,
        value: 42,
    },
    EntryData {
        freq: 5,
        last_tick: 41,
        key: 12,
        value: 12,
    },
    EntryData {
        freq: 10,
        last_tick: 39,
        key: 2,
        value: 2,
    },
    EntryData {
        freq: 19,
        last_tick: 51,
        key: 11,
        value: 11,
    },
]
"
        );
    }

    #[test]
    fn eviction() {
        let mut cache = LfuCacheU64::with_capacity(8);
        for i in 0..8 {
            cache.put(i, i);
            for _ in 0..i {
                cache.get(&i);
            }
        }

        k9::assert_equal!(cache.len(), 8);
        cache.put(8, 8);
        k9::assert_equal!(cache.len(), 8);

        let freq = frequency_order(&cache);
        k9::assert_equal!(*freq[0].key, 8, "0 got evicted, so 8 is first");
        k9::snapshot!(
            freq,
            "
[
    EntryData {
        freq: 0,
        last_tick: 37,
        key: 8,
        value: 8,
    },
    EntryData {
        freq: 1,
        last_tick: 3,
        key: 1,
        value: 1,
    },
    EntryData {
        freq: 2,
        last_tick: 6,
        key: 2,
        value: 2,
    },
    EntryData {
        freq: 3,
        last_tick: 10,
        key: 3,
        value: 3,
    },
    EntryData {
        freq: 4,
        last_tick: 15,
        key: 4,
        value: 4,
    },
    EntryData {
        freq: 5,
        last_tick: 21,
        key: 5,
        value: 5,
    },
    EntryData {
        freq: 6,
        last_tick: 28,
        key: 6,
        value: 6,
    },
    EntryData {
        freq: 7,
        last_tick: 36,
        key: 7,
        value: 7,
    },
]
"
        );

        for i in 9..12 {
            cache.put(i, i);
            cache.get(&i);
        }
        k9::snapshot!(
            frequency_order(&cache),
            "
[
    EntryData {
        freq: 1,
        last_tick: 39,
        key: 9,
        value: 9,
    },
    EntryData {
        freq: 1,
        last_tick: 41,
        key: 10,
        value: 10,
    },
    EntryData {
        freq: 1,
        last_tick: 10,
        key: 3,
        value: 3,
    },
    EntryData {
        freq: 1,
        last_tick: 43,
        key: 11,
        value: 11,
    },
    EntryData {
        freq: 4,
        last_tick: 15,
        key: 4,
        value: 4,
    },
    EntryData {
        freq: 5,
        last_tick: 21,
        key: 5,
        value: 5,
    },
    EntryData {
        freq: 6,
        last_tick: 28,
        key: 6,
        value: 6,
    },
    EntryData {
        freq: 7,
        last_tick: 36,
        key: 7,
        value: 7,
    },
]
"
        );
    }

    #[test]
    fn basic() {
        let mut cache = LfuCacheU64::<&'static str>::with_capacity(8);
        cache.put(1, "hello");
        cache.put(2, "there");

        k9::snapshot!(
            frequency_order(&cache),
            r#"
[
    EntryData {
        freq: 0,
        last_tick: 1,
        key: 1,
        value: "hello",
    },
    EntryData {
        freq: 0,
        last_tick: 2,
        key: 2,
        value: "there",
    },
]
"#
        );

        cache.get(&1);
        cache.get(&1);
        cache.get(&1);
        cache.get(&2);

        k9::snapshot!(
            frequency_order(&cache),
            r#"
[
    EntryData {
        freq: 1,
        last_tick: 6,
        key: 2,
        value: "there",
    },
    EntryData {
        freq: 3,
        last_tick: 5,
        key: 1,
        value: "hello",
    },
]
"#
        );

        k9::snapshot!(
            recency_order(&cache),
            r#"
[
    EntryData {
        freq: 1,
        last_tick: 6,
        key: 2,
        value: "there",
    },
    EntryData {
        freq: 3,
        last_tick: 5,
        key: 1,
        value: "hello",
    },
]
"#
        );

        cache.get(&1);
        k9::snapshot!(
            recency_order(&cache),
            r#"
[
    EntryData {
        freq: 4,
        last_tick: 7,
        key: 1,
        value: "hello",
    },
    EntryData {
        freq: 1,
        last_tick: 6,
        key: 2,
        value: "there",
    },
]
"#
        );
    }
}
