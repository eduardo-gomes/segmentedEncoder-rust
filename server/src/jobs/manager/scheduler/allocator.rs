//!Allocata and deallocate tasks with RAII idiom
//!
//! This provides a HashMap to weak references.
//! When the weakly referenced object gets dropped, the associated entry in the map is removed.

use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Deref;
use std::sync::{Arc, Weak};

use futures::executor::block_on;
use tokio::sync::RwLock;
use uuid::Uuid;

struct WeakMapEntry<T> {
	value: T,
	id: Uuid,
	map: Weak<RwLock<HashMap<Uuid, Weak<WeakMapEntry<T>>>>>,
}
mod debug {
	use std::fmt::{Debug, Formatter};

	use crate::jobs::manager::scheduler::allocator::WeakMapEntry;
	use crate::jobs::manager::scheduler::WeakMapEntryArc;

	impl<T> Debug for WeakMapEntry<T>
	where
		T: Debug,
	{
		fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
			f.debug_struct("WeakMapEntry")
				.field("id", &self.id)
				.field("value", &self.value)
				.finish()
		}
	}

	impl<T> Debug for WeakMapEntryArc<T>
	where
		T: Debug,
	{
		fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
			f.write_str("WeakMapEntryArc: ")?;
			self.0.fmt(f)?;
			f.write_str("\n")
		}
	}
}

impl<T> Drop for WeakMapEntry<T> {
	fn drop(&mut self) {
		eprintln!("Dropping map entry with id: {}", &self.id);
		if let Some(map) = self.map.upgrade() {
			block_on(map.write()).remove(&self.id);
		}
	}
}

pub struct WeakMapEntryArc<T>(Arc<WeakMapEntry<T>>);

impl<T> Clone for WeakMapEntryArc<T> {
	fn clone(&self) -> Self {
		Self(self.0.clone())
	}
}

impl<T> Deref for WeakMapEntryArc<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.0.value
	}
}

#[derive(Debug)]
pub struct WeakUuidMap<T> {
	map: Arc<RwLock<HashMap<Uuid, Weak<WeakMapEntry<T>>>>>,
}

impl<T> WeakUuidMap<T> {
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
	pub async fn insert(&self, value: T) -> (Uuid, WeakMapEntryArc<T>) {
		let id = Uuid::new_v4();
		let entry = Arc::new(WeakMapEntry {
			value,
			map: Arc::downgrade(&self.map),
			id,
		});
		self.map.write().await.insert(id, Arc::downgrade(&entry));
		(id, WeakMapEntryArc(entry))
	}
	pub async fn get(&self, id: &Uuid) -> Option<WeakMapEntryArc<T>> {
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
	use std::ptr;

	use uuid::Uuid;

	use crate::jobs::manager::scheduler::allocator::WeakUuidMap;

	#[tokio::test]
	async fn new_weak_map_len_is_zero_and_is_empty() {
		let weak_map = WeakUuidMap::<()>::new();
		assert_eq!(weak_map.len().await, 0);
		assert!(weak_map.is_empty().await);
	}

	#[tokio::test]
	async fn weak_map_insert_arc_returns_uuid() {
		let weak_map = WeakUuidMap::<()>::new();
		let (id, _arc): (Uuid, _) = weak_map.insert(()).await;
		assert!(!id.is_nil());
	}

	#[tokio::test]
	async fn weak_map_after_insert_is_not_empty() {
		let weak_map = WeakUuidMap::<()>::new();
		let (_, _arc) = weak_map.insert(()).await;
		assert!(!weak_map.is_empty().await);
		assert_eq!(weak_map.len().await, 1);
	}

	#[tokio::test]
	async fn weak_map_get_unknow_id_returns_none() {
		let weak_map = WeakUuidMap::<()>::new();
		let id = Uuid::new_v4();

		let got = weak_map.get(&id).await;
		assert!(got.is_none());
	}

	#[tokio::test]
	async fn weak_map_get_arc_from_id() {
		let weak_map = WeakUuidMap::<()>::new();
		let (id, arc) = weak_map.insert(()).await;

		let got = weak_map.get(&id).await.expect("Should get");
		assert!(ptr::eq(&*got, &*arc));
	}

	#[tokio::test]
	async fn weak_map_after_droping_arc_is_empty() {
		let weak_map = WeakUuidMap::<()>::new();
		let (_, arc) = weak_map.insert(()).await;
		drop(arc);
		assert!(weak_map.is_empty().await);
	}
}
