//! A Python dict whose order is printed or written (02-interfaces.md, "Python dicts").

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use std::fmt;
use std::marker::PhantomData;

/// A Python dict whose order is printed or written: entries in insertion order; `insert` of a present key replaces
/// its value in place. Serialised as a JSON object in that order; deserialised keeping the file's order, a repeated
/// key keeping its first place and taking the last value (as json.loads builds a dict). `==` compares in order
/// (Python's dict == does not: sort first where order may differ).
#[derive(Clone, Debug, PartialEq)]
pub struct Ordered<V>(pub Vec<(String, V)>);

/// Empty; written by hand so that V needs no Default.
impl<V> Default for Ordered<V> {
    fn default() -> Self {
        Ordered(Vec::new())
    }
}

impl<V> Ordered<V> {
    pub fn new() -> Ordered<V> {
        Ordered(Vec::new())
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        self.0.iter().find(|(name, _)| name == key).map(|(_, value)| value)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn insert(&mut self, key: String, value: V) {
        match self.0.iter().position(|(name, _)| *name == key) {
            Some(at) => self.0[at].1 = value,
            None => self.0.push((key, value)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.0.iter().map(|(name, value)| (name.as_str(), value))
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<V: serde::Serialize> serde::Serialize for Ordered<V> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, value) in &self.0 {
            map.serialize_entry(name, value)?;
        }
        map.end()
    }
}

impl<'de, V: serde::Deserialize<'de>> serde::Deserialize<'de> for Ordered<V> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OrderedVisitor<V>(PhantomData<V>);

        impl<'de, V: serde::Deserialize<'de>> Visitor<'de> for OrderedVisitor<V> {
            type Value = Ordered<V>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Ordered<V>, A::Error> {
                let mut out = Ordered::new();
                while let Some((name, value)) = access.next_entry::<String, V>()? {
                    out.insert(name, value);
                }
                Ok(out)
            }
        }

        deserializer.deserialize_map(OrderedVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::Ordered;

    #[test]
    fn insert_replaces_a_present_key_in_place() {
        let mut map = Ordered::new();
        map.insert("b".to_string(), 1);
        map.insert("a".to_string(), 2);
        map.insert("b".to_string(), 3);
        assert_eq!(map.0, vec![("b".to_string(), 3), ("a".to_string(), 2)]);
        assert_eq!(map.get("b"), Some(&3));
        assert!(map.contains_key("a"));
        assert!(!map.contains_key("c"));
        assert_eq!(map.iter().collect::<Vec<_>>(), vec![("b", &3), ("a", &2)]);
        assert_eq!(map.len(), 2);
        assert!(!map.is_empty());
        assert!(Ordered::<i64>::default().is_empty());
    }

    #[test]
    fn serde_keeps_the_file_order_and_a_repeated_key_its_first_place() {
        let map: Ordered<i64> = serde_json::from_str(r#"{"b": 1, "a": 2, "b": 3}"#).unwrap();
        assert_eq!(map.0, vec![("b".to_string(), 3), ("a".to_string(), 2)]);
        assert_eq!(serde_json::to_string(&map).unwrap(), r#"{"b":3,"a":2}"#);
        assert!(serde_json::from_str::<Ordered<i64>>("[1]").is_err());
    }
}
