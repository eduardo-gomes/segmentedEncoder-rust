use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

struct TimedMapEntry<Value>(AtomicU64, Value);

/// A map that let you remove entries after some time without updates.
/// This struct wont remove elements automatically, but only when requested
pub struct TimedMap<Key, Val>
where
	Key: Eq + Hash,
{
	map: HashMap<Key, TimedMapEntry<Val>>,
}

impl<Key, Val> TimedMap<Key, Val>
where
	Key: Eq + Hash,
{
	pub fn new() -> Self {
		TimedMap {
			map: HashMap::new(),
		}
	}

	pub fn is_empty(&self) -> bool {
		self.map.is_empty()
	}

	pub fn insert(&mut self, key: Key, value: Val) {
		let now = AtomicU64::from(Self::now_secs());
		self.map.insert(key, TimedMapEntry(now, value));
	}
	pub fn remove<Q>(&mut self, key: &Q)
	where
		Key: Borrow<Q>,
		Q: Hash + Eq + ?Sized,
	{
		self.map.remove(key);
	}
	pub fn get<Q>(&self, key: &Q) -> Option<&Val>
	where
		Key: Borrow<Q>,
		Q: Hash + Eq + ?Sized,
	{
		self.map.get(key).map(|entry| &entry.1)
	}

	fn now_secs() -> u64 {
		let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH);
		now.map(|duration| duration.as_secs()) //Will error if system time is set before unix epoch
			.unwrap_or(Default::default())
	}

	//This function has seconds precision
	pub(crate) fn timeout(&mut self, duration: Duration) {
		fn timestamp_from_entry(val: &AtomicU64) -> u64 {
			val.load(Ordering::Acquire)
		}
		let expired = Self::now_secs();
		self.map
			.retain(|_, entry| timestamp_from_entry(&entry.0) >= expired)
	}
}

#[cfg(test)]
mod test {
	use std::thread::sleep;

	use crate::TimedMap;

	#[test]
	fn new_timed_map_is_empty() {
		let map: TimedMap<String, String> = TimedMap::new();
		assert!(map.is_empty())
	}

	#[test]
	fn new_timed_map_is_not_empty_after_insert() {
		let mut map = TimedMap::new();
		let key = "KEY";
		let value = "Value";
		map.insert(key.to_string(), value.to_string());
		assert!(!map.is_empty())
	}

	#[test]
	fn new_timed_map_get_after_insert() {
		let mut map = TimedMap::new();
		let key = "KEY";
		let value = "Value";
		map.insert(key.to_string(), value.to_string());
		let got = map.get(key).expect("Should get the stored value");
		assert_eq!(got, value)
	}

	#[test]
	fn new_timed_map_after_remove_is_empty() {
		let mut map = TimedMap::new();
		let key = "KEY";
		let value = "Value";
		map.insert(key.to_string(), value.to_string());
		map.remove(key);
		assert!(map.is_empty(), "Element should be removed")
	}

	#[test]
	fn new_timed_map_insert_get_other_type() {
		let mut map = TimedMap::new();
		let key = 123456;
		let value = 789;
		map.insert(key.clone(), value.clone());
		let got = map.get(&key).expect("Should get the stored value");
		assert_eq!(got, &value)
	}

	#[test]
	fn new_timed_map_timeout_without_delay_wont_remove() {
		let mut map = TimedMap::new();
		let key = "KEY";
		let value = "Value";
		map.insert(key, value);

		let timeout = std::time::Duration::from_secs(1);
		map.timeout(timeout);

		assert!(!map.is_empty())
	}

	#[test]
	fn new_timed_map_timeout_with_delay_will_remove() {
		let mut map = TimedMap::new();
		let key = "KEY";
		let value = "Value";
		map.insert(key, value);

		let timeout = std::time::Duration::from_secs(1);
		sleep(timeout);
		map.timeout(timeout);

		assert!(map.is_empty(), "Timeout should remove after the sleep")
	}
}
