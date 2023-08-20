use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Sub;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

struct AtomicTimestamp(AtomicU64);

fn timestamp_now() -> Duration {
	SystemTime::now()
		.duration_since(SystemTime::UNIX_EPOCH)
		.unwrap_or(Default::default())
}

impl AtomicTimestamp {
	fn now() -> Self {
		let now = timestamp_now().as_secs();
		Self(AtomicU64::from(now))
	}
	fn load(&self) -> Duration {
		let val = self.0.load(Ordering::Acquire);
		Duration::from_secs(val)
	}

	fn set_to_now(&self) {
		let now = timestamp_now().as_secs();
		self.0.store(now, Ordering::Release);
	}
}

struct TimedMapEntry<Value>(AtomicTimestamp, Value);

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

	/// Insert new element into map, and set its timestamp to current time
	pub fn insert(&mut self, key: Key, value: Val) {
		let now = AtomicTimestamp::now();
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
		self.map.get(key).map(|entry| {
			entry.0.set_to_now();
			&entry.1
		})
	}

	pub(crate) fn timeout(&mut self, duration: Duration) {
		let expired = timestamp_now() - duration;
		self.map.retain(|_, entry| entry.0.load() >= expired)
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

	#[test]
	fn new_timed_map_timeout_after_get_should_not_remove() {
		let mut map = TimedMap::new();
		let key = "KEY";
		let value = "Value";
		map.insert(key, value);

		let timeout = std::time::Duration::from_secs(1);
		sleep(timeout);
		map.get(key).expect("Should get");
		map.timeout(timeout);

		assert!(
			!map.is_empty(),
			"Timeout should not remove because the get updates the timestamp"
		)
	}
}
