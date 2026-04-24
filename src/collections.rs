use bitcoin::hashes::Hash;
use std::collections::{BTreeMap, HashMap};

use crate::partial_join::PartialJoin;

impl<V> PartialJoin for Option<V>
where
    V: PartialJoin + Clone,
{
    type Error = V::Error;

    fn join(&self, other: &Self) -> Result<Self, Self::Error> {
        Ok(match (self, other) {
            (None, None) => None,
            (None, x) | (x, None) => x.clone(),
            (Some(a), Some(b)) => Some(a.join(b)?),
        })
    }
}

impl<A, B> PartialJoin for (A, B)
where
    A: PartialJoin + Clone,
    B: PartialJoin<Error = A::Error> + Clone,
{
    type Error = A::Error;
    fn join(&self, other: &Self) -> Result<Self, Self::Error> {
        Ok((self.0.join(&other.0)?, self.1.join(&other.1)?))
    }
}

impl<K, V> PartialJoin for BTreeMap<K, V>
where
    K: Ord + Clone,
    V: PartialJoin + Clone,
{
    type Error = V::Error;

    fn join(&self, other: &Self) -> Result<Self, Self::Error> {
        let mut new = BTreeMap::new();

        for (k, b) in self.iter().chain(other) {
            use std::collections::btree_map::Entry::*;
            match new.entry(k.clone()) {
                Occupied(mut entry) => {
                    let a: &mut V = entry.get_mut();
                    *a = a.join(b)?;
                }
                Vacant(entry) => {
                    entry.insert(b.clone());
                }
            }
        }

        Ok(new)
    }
}

impl<K, V> PartialJoin for HashMap<K, V>
where
    K: Hash + Clone,
    V: PartialJoin + Clone,
{
    type Error = V::Error;

    fn join<'a>(&'a self, other: &'a Self) -> Result<Self, Self::Error> {
        let mut new = HashMap::new();

        for (k, b) in self.iter().chain(other) {
            use std::collections::hash_map::Entry::*;
            match new.entry(k.clone()) {
                Occupied(mut entry) => {
                    let a: &mut V = entry.get_mut();
                    *a = a.join(b)?;
                }
                Vacant(entry) => {
                    entry.insert(b.clone());
                }
            }
        }

        Ok(new)
    }
}

use crate::values::ValueError;
impl<V> PartialJoin for Vec<V>
where
    V: PartialJoin<Error = ValueError> + PartialEq + Clone,
{
    type Error = ValueError;

    fn join(&self, other: &Self) -> Result<Self, Self::Error> {
        if self == other {
            return Ok(self.clone());
        };

        _ = self.len().join(&other.len())?;

        std::iter::zip(self, other)
            .map(|(x, y)| x.join(y))
            .collect()
    }
}
