use config::TextStyle;

#[derive(PartialEq, Eq, Hash)]
pub struct ShapeCacheKey {
    pub style: TextStyle,
    pub text: String,
}

/// We'd like to avoid allocating when resolving from the cache
/// so this is the borrowed version of ShapeCacheKey.
/// It's a bit involved to make this work; more details can be
/// found in the excellent guide here:
/// <https://github.com/sunshowers/borrow-complex-key-example/blob/master/src/lib.rs>
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct BorrowedShapeCacheKey<'a> {
    pub style: &'a TextStyle,
    pub text: &'a str,
}

impl<'a> BorrowedShapeCacheKey<'a> {
    pub fn to_owned(&self) -> ShapeCacheKey {
        ShapeCacheKey {
            style: self.style.clone(),
            text: self.text.to_owned(),
        }
    }
}

pub trait ShapeCacheKeyTrait {
    fn key<'k>(&'k self) -> BorrowedShapeCacheKey<'k>;
}

impl ShapeCacheKeyTrait for ShapeCacheKey {
    fn key<'k>(&'k self) -> BorrowedShapeCacheKey<'k> {
        BorrowedShapeCacheKey {
            style: &self.style,
            text: &self.text,
        }
    }
}

impl<'a> ShapeCacheKeyTrait for BorrowedShapeCacheKey<'a> {
    fn key<'k>(&'k self) -> BorrowedShapeCacheKey<'k> {
        *self
    }
}

impl<'a> std::borrow::Borrow<dyn ShapeCacheKeyTrait + 'a> for ShapeCacheKey {
    fn borrow(&self) -> &(dyn ShapeCacheKeyTrait + 'a) {
        self
    }
}

impl<'a> std::borrow::Borrow<dyn ShapeCacheKeyTrait + 'a> for lru::KeyRef<ShapeCacheKey> {
    fn borrow(&self) -> &(dyn ShapeCacheKeyTrait + 'a) {
        let k: &ShapeCacheKey = self.borrow();
        k
    }
}

impl<'a> PartialEq for (dyn ShapeCacheKeyTrait + 'a) {
    fn eq(&self, other: &Self) -> bool {
        self.key().eq(&other.key())
    }
}

impl<'a> Eq for (dyn ShapeCacheKeyTrait + 'a) {}

impl<'a> std::hash::Hash for (dyn ShapeCacheKeyTrait + 'a) {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key().hash(state)
    }
}
