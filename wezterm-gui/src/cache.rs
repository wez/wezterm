pub struct LruCache<K, V> {
    hit: &'static str,
    miss: &'static str,
    cache: lru::LruCache<K, V>,
}

impl<K: std::hash::Hash + std::cmp::Eq, V> LruCache<K, V> {
    pub fn new(hit: &'static str, miss: &'static str, cap: usize) -> Self {
        Self {
            hit,
            miss,
            cache: lru::LruCache::new(cap),
        }
    }

    pub fn get<'a, Q: ?Sized>(&'a mut self, k: &Q) -> Option<&'a V>
    where
        lru::KeyRef<K>: std::borrow::Borrow<Q>,
        Q: std::hash::Hash + Eq,
    {
        let result = self.cache.get(k);
        metrics::histogram!(
            if result.is_some() {
                self.hit
            } else {
                self.miss
            },
            1.
        );
        result
    }
}

impl<K, V> std::ops::Deref for LruCache<K, V> {
    type Target = lru::LruCache<K, V>;
    fn deref(&self) -> &lru::LruCache<K, V> {
        &self.cache
    }
}

impl<K, V> std::ops::DerefMut for LruCache<K, V> {
    fn deref_mut(&mut self) -> &mut lru::LruCache<K, V> {
        &mut self.cache
    }
}
