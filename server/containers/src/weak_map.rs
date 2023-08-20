//! WeakMap implementation, a map that erases entries when its references get destructed

use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::{Arc, Weak};

use tokio::sync::RwLock;

/// [WeakMap] entry, when this object is destructed, it removes itself from the map
struct WeakMapEntry<Key, V>
where
	Key: Clone + Eq + Hash,
{
	value: V,
	id: Key,
	map: Weak<RwLock<HashMap<Key, Weak<Self>>>>,
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
			tokio::task::block_in_place(move || {
				tokio::runtime::Handle::current()
					.block_on(map.write())
					.remove(&self.id)
			});
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
	map: Arc<RwLock<HashMap<Key, Weak<WeakMapEntry<Key, V>>>>>,
}

impl<Key, V> WeakMap<Key, V>
where
	Key: Clone + Eq + Hash,
{
	pub fn new() -> Self {
		Self {
			map: Default::default(),
		}
	}
	pub async fn len(&self) -> usize {
		self.map.read().await.len()
	}
	pub async fn is_empty(&self) -> bool {
		self.map.read().await.is_empty()
	}
	pub async fn insert(&self, key: Key, value: V) -> (Key, WeakMapEntryArc<Key, V>) {
		let entry = Arc::new(WeakMapEntry {
			value,
			map: Arc::downgrade(&self.map),
			id: key.clone(),
		});
		self.map
			.write()
			.await
			.insert(key.clone(), Arc::downgrade(&entry));
		(key, WeakMapEntryArc(entry))
	}
	pub async fn get(&self, id: &Key) -> Option<WeakMapEntryArc<Key, V>> {
		self.map
			.read()
			.await
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

	#[tokio::test(flavor = "multi_thread")]
	async fn weak_map_insert_arc_returns_reference() {
		let weak_map = WeakMap::<u64, _>::new();
		let value = 123456789;
		let (_id, arc) = weak_map.insert(123, value).await;
		assert_eq!(arc.deref(), &value);
	}

	#[tokio::test(flavor = "multi_thread")]
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

	#[tokio::test(flavor = "multi_thread")]
	async fn weak_map_get_arc_from_id() {
		let weak_map = WeakMap::<u64, ()>::new();
		let (id, arc) = weak_map.insert(123, ()).await;

		let got = weak_map.get(&id).await.expect("Should get");
		assert!(ptr::eq(&*got, &*arc));
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn weak_map_after_droping_arc_is_empty() {
		let weak_map = WeakMap::<u64, ()>::new();
		let (_, arc) = weak_map.insert(123, ()).await;
		drop(arc);
		assert!(weak_map.is_empty().await);
	}
}
