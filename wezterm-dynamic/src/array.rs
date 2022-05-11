use crate::Value;
use core::iter::FromIterator;
use core::ops::{Deref, DerefMut};
use std::cmp::Ordering;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Array {
    inner: Vec<Value>,
}

impl Ord for Array {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_ptr = self as *const Self;
        let other_ptr = other as *const Self;
        self_ptr.cmp(&other_ptr)
    }
}

impl PartialOrd for Array {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<Vec<Value>> for Array {
    fn from(inner: Vec<Value>) -> Self {
        Self { inner }
    }
}

impl Drop for Array {
    fn drop(&mut self) {
        self.inner.drain(..).for_each(crate::drop::safely);
    }
}

fn take(array: Array) -> Vec<Value> {
    let array = core::mem::ManuallyDrop::new(array);
    unsafe { core::ptr::read(&array.inner) }
}

impl Array {
    pub fn new() -> Self {
        Array { inner: Vec::new() }
    }
}

impl Deref for Array {
    type Target = Vec<Value>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Array {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl IntoIterator for Array {
    type Item = Value;
    type IntoIter = <Vec<Value> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        take(self).into_iter()
    }
}

impl<'a> IntoIterator for &'a Array {
    type Item = &'a Value;
    type IntoIter = <&'a Vec<Value> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Array {
    type Item = &'a mut Value;
    type IntoIter = <&'a mut Vec<Value> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl FromIterator<Value> for Array {
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = Value>,
    {
        Array {
            inner: Vec::from_iter(iter),
        }
    }
}
