//! WeakMap implementation, a map that erases entries when its references get destructed

use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::RwLock;
use std::sync::{Arc, RwLockReadGuard, RwLockWriteGuard, Weak};

/// [WeakMap] entry, when this object is destructed, it removes itself from the map
struct WeakMapEntry<Key, V>
where
	Key: Clone + Eq + Hash,
{
	value: V,
	id: Key,
	map: Weak<WeakMap<Key, V>>,
}

mod debug {
	use std::fmt::{Debug, Formatter};
	use std::hash::Hash;

	use super::WeakMapEntry;
	use super::WeakMapEntryArc;

	impl<Key, V> Debug for WeakMapEntry<Key, V>
	where
		Key: Clone + Eq + Hash,
		Key: Debug,
		V: Debug,
	{
		fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
			f.debug_struct("WeakMapEntry")
				.field("id", &self.id)
				.field("value", &self.value)
				.finish()
		}
	}

	impl<Key, V> Debug for WeakMapEntryArc<Key, V>
	where
		Key: Clone + Eq + Hash,
		Key: Debug,
		V: Debug,
	{
		fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
			f.write_str("WeakMapEntryArc: ")?;
			self.0.fmt(f)?;
			f.write_str("\n")
		}
	}
}

impl<Key, V> Drop for WeakMapEntry<Key, V>
where
	Key: Clone + Eq + Hash,
{
	fn drop(&mut self) {
		if let Some(map) = self.map.upgrade() {
			map.map
				.write()
				.unwrap_or_else(|err| err.into_inner())
				.remove(&self.id);
		}
	}
}

pub struct WeakMapEntryArc<Key, V>(Arc<WeakMapEntry<Key, V>>)
where
	Key: Clone + Eq + Hash;

impl<Key, V> Clone for WeakMapEntryArc<Key, V>
where
	Key: Clone + Eq + Hash,
{
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<Key, V> Deref for WeakMapEntryArc<Key, V>
where
	Key: Clone + Eq + Hash,
{
	type Target = V;

	fn deref(&self) -> &Self::Target {
		&self.0.value
	}
}

#[derive(Debug)]
pub struct WeakMap<Key, V>
where
	Key: Clone + Eq + Hash,
{
	map: RwLock<HashMap<Key, Weak<WeakMapEntry<Key, V>>>>,
}

impl<Key, V> WeakMap<Key, V>
where
	Key: Clone + Eq + Hash,
{
	pub fn new() -> Arc<Self> {
		Arc::new(Self {
			map: Default::default(),
		})
	}

	fn sync_read(&self) -> RwLockReadGuard<'_, HashMap<Key, Weak<WeakMapEntry<Key, V>>>> {
		self.map.read().unwrap_or_else(|err| err.into_inner())
	}
	fn sync_write(&self) -> RwLockWriteGuard<'_, HashMap<Key, Weak<WeakMapEntry<Key, V>>>> {
		self.map.write().unwrap_or_else(|err| err.into_inner())
	}
	pub async fn len(&self) -> usize {
		self.sync_read().len()
	}
	pub async fn is_empty(&self) -> bool {
		self.sync_read().is_empty()
	}
	pub async fn insert(self: &Arc<Self>, key: Key, value: V) -> (Key, WeakMapEntryArc<Key, V>) {
		let entry = Arc::new(WeakMapEntry {
			value,
			map: Arc::downgrade(self),
			id: key.clone(),
		});
		self.sync_write()
			.insert(key.clone(), Arc::downgrade(&entry));
		(key, WeakMapEntryArc(entry))
	}
	pub async fn get(&self, id: &Key) -> Option<WeakMapEntryArc<Key, V>> {
		self.sync_read()
			.get(id)
			.and_then(|weak| weak.upgrade())
			.map(WeakMapEntryArc)
	}
}

#[cfg(test)]
mod test {
	use std::ops::Deref;
	use std::ptr;

	use super::WeakMap;

	#[tokio::test]
	async fn new_weak_map_len_is_zero_and_is_empty() {
		let weak_map = WeakMap::<u64, ()>::new();
		assert_eq!(weak_map.len().await, 0);
		assert!(weak_map.is_empty().await);
	}

	#[tokio::test]
	async fn weak_map_insert_arc_returns_reference() {
		let weak_map = WeakMap::<u64, _>::new();
		let value = 123456789;
		let (_id, arc) = weak_map.insert(123, value).await;
		assert_eq!(arc.deref(), &value);
	}

	#[tokio::test]
	async fn weak_map_after_insert_is_not_empty() {
		let weak_map = WeakMap::<u64, ()>::new();
		let (_, _arc) = weak_map.insert(123, ()).await;
		assert!(!weak_map.is_empty().await);
		assert_eq!(weak_map.len().await, 1);
	}

	#[tokio::test]
	async fn weak_map_get_unknow_id_returns_none() {
		let weak_map = WeakMap::<u64, ()>::new();
		let id = 123;

		let got = weak_map.get(&id).await;
		assert!(got.is_none());
	}

	#[tokio::test]
	async fn weak_map_get_arc_from_id() {
		let weak_map = WeakMap::<u64, ()>::new();
		let (id, arc) = weak_map.insert(123, ()).await;

		let got = weak_map.get(&id).await.expect("Should get");
		assert!(ptr::eq(&*got, &*arc));
	}

	#[tokio::test]
	async fn weak_map_after_dropping_arc_is_empty() {
		let weak_map = WeakMap::<u64, ()>::new();
		let (_, arc) = weak_map.insert(123, ()).await;
		drop(arc);
		assert!(weak_map.is_empty().await);
	}
}
