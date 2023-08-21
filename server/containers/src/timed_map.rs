use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

/// Type to store [Duration] atomically. May be slightly of due to concurrency
struct AtomicTimestamp(AtomicU64, AtomicU32);

fn timestamp_now() -> Duration {
	SystemTime::now()
		.duration_since(SystemTime::UNIX_EPOCH)
		.unwrap_or(Default::default())
}

impl AtomicTimestamp {
	fn duration_to_u64u32(val: &Duration) -> (u64, u32) {
		(val.as_secs(), val.subsec_nanos())
	}
	fn u64u32_to_duration(secs: u64, nanos: u32) -> Duration {
		Duration::new(secs, nanos)
	}
	fn now() -> Self {
		let now = timestamp_now();
		let (secs, nanos) = Self::duration_to_u64u32(&now);
		Self(AtomicU64::from(secs), AtomicU32::from(nanos))
	}
	fn load(&self) -> Duration {
		let secs = self.0.load(Ordering::Acquire);
		let nanos = self.1.load(Ordering::Acquire);
		Self::u64u32_to_duration(secs, nanos)
	}

	fn store(&self, duration: &Duration) {
		let (secs, nanos) = Self::duration_to_u64u32(duration);
		self.0.store(secs, Ordering::Release);
		self.1.store(nanos, Ordering::Release);
	}

	fn set_to_now(&self) {
		self.store(&timestamp_now());
	}
}

struct TimedMapEntry<Value>(AtomicTimestamp, Value);

/// A map that let you remove entries after some time without updates.
/// This struct won't remove elements automatically, but only when requested
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
	/// Creates an empty TimedMap.
	pub fn new() -> Self {
		TimedMap {
			map: HashMap::new(),
		}
	}

	///Returns true if the map contains no elements.
	pub fn is_empty(&self) -> bool {
		self.map.is_empty()
	}

	/// Insert new element into map, and set its timestamp to current time.
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

	/// Returns a reference to the stored element, and update it's timestamp to now.
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

	/// Removes all elements that have not been accessed for a period greater than [duration](Duration).
	pub fn timeout(&mut self, duration: Duration) {
		let expired = timestamp_now() - duration;
		self.map.retain(|_, entry| entry.0.load() >= expired)
	}
}

#[cfg(test)]
mod test {
	use std::thread::sleep;
	use std::time::Duration;

	use super::TimedMap;

	const TEST_TIMEOUT: Duration = Duration::from_millis(10);

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

		let timeout = TEST_TIMEOUT;
		map.timeout(timeout);

		assert!(!map.is_empty())
	}

	#[test]
	fn new_timed_map_timeout_with_delay_will_remove() {
		let mut map = TimedMap::new();
		let key = "KEY";
		let value = "Value";
		map.insert(key, value);

		let timeout = TEST_TIMEOUT;
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

		let timeout = TEST_TIMEOUT;
		sleep(timeout);
		map.get(key).expect("Should get");
		map.timeout(timeout);

		assert!(
			!map.is_empty(),
			"Timeout should not remove because the get updates the timestamp"
		)
	}
}
