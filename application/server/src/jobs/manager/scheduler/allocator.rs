//!Allocate and deallocate tasks with RAII idiom
//!
//! This provides a HashMap to weak references.
//! When the weakly referenced object gets dropped, the associated entry in the map is removed.

use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use containers::{WeakMap, WeakMapEntryArc as EntryArc};

#[async_trait]
pub trait WeakUuidMapRandomKey<V> {
	async fn insert_random_key(&self, value: V) -> (Uuid, EntryArc<Uuid, V>);
}

#[async_trait]
impl<V: Send + Sync> WeakUuidMapRandomKey<V> for Arc<WeakUuidMap<V>> {
	async fn insert_random_key(&self, value: V) -> (Uuid, EntryArc<Uuid, V>) {
		self.insert(Uuid::new_v4(), value).await
	}
}

/// A map that holds its objects while their [Weak] reference is valid
pub type WeakUuidMap<T> = WeakMap<Uuid, T>;
/// A type of [Arc] that removes the object from the WeakMap when de object gets deallocated
pub type WeakMapEntryArc<T> = EntryArc<Uuid, T>;

#[cfg(test)]
mod test {
	use uuid::Uuid;

	use crate::jobs::manager::scheduler::allocator::{WeakUuidMap, WeakUuidMapRandomKey};

	#[tokio::test]
	async fn weak_map_insert_random_key_returns_uuid() {
		let weak_map = WeakUuidMap::<()>::new();
		let (id, _arc): (Uuid, _) = weak_map.insert_random_key(()).await;
		assert!(!id.is_nil());
	}

	#[tokio::test]
	async fn weak_map_get_unknown_id_returns_none() {
		let weak_map = WeakUuidMap::<()>::new();
		let id = Uuid::new_v4();

		let got = weak_map.get(&id).await;
		assert!(got.is_none());
	}

	#[tokio::test]
	async fn weak_map_after_dropping_arc_is_empty() {
		let weak_map = WeakUuidMap::<()>::new();
		let (_, arc) = weak_map.insert_random_key(()).await;
		drop(arc);
		assert!(weak_map.is_empty().await);
	}
}
