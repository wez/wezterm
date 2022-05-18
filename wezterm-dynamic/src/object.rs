use crate::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};

/// We'd like to avoid allocating when resolving struct fields,
/// so this is the borrowed version of Value.
/// It's a bit involved to make this work; more details can be
/// found in the excellent guide here:
/// <https://github.com/sunshowers/borrow-complex-key-example/blob/master/src/lib.rs>
#[derive(Copy, Clone, Debug, PartialEq, Hash, Eq, Ord, PartialOrd)]
pub enum BorrowedKey<'a> {
    Value(&'a Value),
    Str(&'a str),
}

pub trait ObjectKeyTrait {
    fn key<'k>(&'k self) -> BorrowedKey<'k>;
}

impl ObjectKeyTrait for Value {
    fn key<'k>(&'k self) -> BorrowedKey<'k> {
        match self {
            Value::String(s) => BorrowedKey::Str(s.as_str()),
            v => BorrowedKey::Value(v),
        }
    }
}

impl<'a> ObjectKeyTrait for BorrowedKey<'a> {
    fn key<'k>(&'k self) -> BorrowedKey<'k> {
        *self
    }
}

impl<'a> std::borrow::Borrow<dyn ObjectKeyTrait + 'a> for Value {
    fn borrow(&self) -> &(dyn ObjectKeyTrait + 'a) {
        self
    }
}

impl<'a> PartialEq for (dyn ObjectKeyTrait + 'a) {
    fn eq(&self, other: &Self) -> bool {
        self.key().eq(&other.key())
    }
}

impl<'a> Eq for (dyn ObjectKeyTrait + 'a) {}

impl<'a> PartialOrd for (dyn ObjectKeyTrait + 'a) {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.key().partial_cmp(&other.key())
    }
}

impl<'a> Ord for (dyn ObjectKeyTrait + 'a) {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key().cmp(&other.key())
    }
}

impl<'a> std::hash::Hash for (dyn ObjectKeyTrait + 'a) {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.key().hash(state)
    }
}

#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct Object {
    inner: BTreeMap<Value, Value>,
}

impl Object {
    pub fn get_by_str(&self, field_name: &str) -> Option<&Value> {
        self.inner
            .get(&BorrowedKey::Str(field_name) as &dyn ObjectKeyTrait)
    }
}

impl std::fmt::Debug for Object {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.inner.fmt(fmt)
    }
}

impl Ord for Object {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_ptr = self as *const Self;
        let other_ptr = other as *const Self;
        self_ptr.cmp(&other_ptr)
    }
}

impl PartialOrd for Object {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        for (_, child) in std::mem::replace(&mut self.inner, BTreeMap::new()) {
            crate::drop::safely(child);
        }
    }
}

impl From<BTreeMap<Value, Value>> for Object {
    fn from(inner: BTreeMap<Value, Value>) -> Self {
        Self { inner }
    }
}

impl Deref for Object {
    type Target = BTreeMap<Value, Value>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Object {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

fn take(object: Object) -> BTreeMap<Value, Value> {
    let object = core::mem::ManuallyDrop::new(object);
    unsafe { core::ptr::read(&object.inner) }
}

impl IntoIterator for Object {
    type Item = (Value, Value);
    type IntoIter = <BTreeMap<Value, Value> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        take(self).into_iter()
    }
}

impl<'a> IntoIterator for &'a Object {
    type Item = (&'a Value, &'a Value);
    type IntoIter = <&'a BTreeMap<Value, Value> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Object {
    type Item = (&'a Value, &'a mut Value);
    type IntoIter = <&'a mut BTreeMap<Value, Value> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl FromIterator<(Value, Value)> for Object {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (Value, Value)>,
    {
        Object {
            inner: BTreeMap::from_iter(iter),
        }
    }
}
