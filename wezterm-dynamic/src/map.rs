use crate::Value;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Map {
    inner: BTreeMap<Value, Value>,
}

impl Ord for Map {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_ptr = self as *const Self;
        let other_ptr = other as *const Self;
        self_ptr.cmp(&other_ptr)
    }
}

impl PartialOrd for Map {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Drop for Map {
    fn drop(&mut self) {
        for (_, child) in std::mem::replace(&mut self.inner, BTreeMap::new()) {
            crate::drop::safely(child);
        }
    }
}

impl From<BTreeMap<Value, Value>> for Map {
    fn from(inner: BTreeMap<Value, Value>) -> Self {
        Self { inner }
    }
}

impl Deref for Map {
    type Target = BTreeMap<Value, Value>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Map {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

fn take(object: Map) -> BTreeMap<Value, Value> {
    let object = core::mem::ManuallyDrop::new(object);
    unsafe { core::ptr::read(&object.inner) }
}

impl IntoIterator for Map {
    type Item = (Value, Value);
    type IntoIter = <BTreeMap<Value, Value> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        take(self).into_iter()
    }
}

impl<'a> IntoIterator for &'a Map {
    type Item = (&'a Value, &'a Value);
    type IntoIter = <&'a BTreeMap<Value, Value> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Map {
    type Item = (&'a Value, &'a mut Value);
    type IntoIter = <&'a mut BTreeMap<Value, Value> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl FromIterator<(Value, Value)> for Map {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (Value, Value)>,
    {
        Map {
            inner: BTreeMap::from_iter(iter),
        }
    }
}
