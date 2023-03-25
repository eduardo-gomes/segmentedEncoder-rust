//!
//! [JobScheduler] will hold a job and all tasks from this job.
//!
//! When a scheduler is created, it will generate all tasks that do not need preprocessing to be
//! created and them will start in the pre-execution or execution phases
//!
//! Jobs will have 3 phases:
//! - **Pre-execution**: Useful to analyze input and generate more tasks to be executed
//! - **Execution**: Runs the encoding
//! - **Post-execution**: Useful to merge all the artifacts into a single file
//!

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::jobs::segmenter::TaskInfo;
use crate::jobs::{Job, Segmenter};

struct ScheduledTaskInfo {
	allocated: AtomicBool,
	task: TaskInfo,
}

impl ScheduledTaskInfo {
	///Check if task is available, and return an [AllocatedTask] tracking this task allocation
	fn allocate(self: &Arc<Self>) -> Option<AllocatedTask> {
		let allocated = self.allocated.swap(true, Ordering::AcqRel);
		match allocated {
			false => Some(AllocatedTask {
				scheduled: self.clone(),
			}),
			true => None,
		}
	}
}

pub(crate) struct JobScheduler {
	job: Arc<Job>,
	tasks: Vec<Arc<ScheduledTaskInfo>>,
	allocated: RwLock<HashMap<Uuid, Weak<AllocatedTask>>>,
}

mod allocator {
	//!Allocata and deallocate tasks with RAII idiom
	//!
	//! This provides a HashMap to weak references.
	//! When the weakly referenced object gets dropped, the associated entry in the map is removed.

	use std::collections::HashMap;
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

	struct WeakMapEntryArc<T>(Arc<WeakMapEntry<T>>);

	impl<T> Drop for WeakMapEntry<T> {
		fn drop(&mut self) {
			if let Some(map) = self.map.upgrade() {
				block_on(async { map.write().await.remove(&self.id) });
			}
		}
	}

	impl<T> Deref for WeakMapEntryArc<T> {
		type Target = T;

		fn deref(&self) -> &Self::Target {
			&self.0.value
		}
	}

	struct WeakUuidMap<T> {
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

		use crate::job_manager::job_scheduler::allocator::WeakUuidMap;

		#[tokio::test]
		async fn new_weak_map_len_is_zero_and_is_empty() {
			let weak_map = WeakUuidMap::<()>::new();
			assert_eq!(weak_map.len().await, 0);
			assert!(weak_map.is_empty().await);
		}

		#[tokio::test]
		async fn weak_map_insert_arc_returns_uuid() {
			let weak_map = WeakUuidMap::<()>::new();
			let (id, arc): (Uuid, _) = weak_map.insert(()).await;
			assert!(!id.is_nil());
		}

		#[tokio::test]
		async fn weak_map_after_insert_is_not_empty() {
			let weak_map = WeakUuidMap::<()>::new();
			let (_, arc) = weak_map.insert(()).await;
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
}

///Marks an allocated task
///
/// This object keeps the task allocated until it is dropped.
/// Also allow access to the allocated task
pub struct AllocatedTask {
	scheduled: Arc<ScheduledTaskInfo>,
}

impl AllocatedTask {
	fn as_task(&self) -> &TaskInfo {
		&self.scheduled.task
	}
}

impl Drop for AllocatedTask {
	fn drop(&mut self) {
		self.scheduled.allocated.store(false, Ordering::Release);
	}
}

impl JobScheduler {
	pub(super) fn new(job: Arc<Job>) -> Self {
		let tasks = Segmenter::segment(job.as_ref())
			.tasks
			.into_iter()
			.map(|info| {
				ScheduledTaskInfo {
					allocated: false.into(),
					task: info,
				}
				.into()
			})
			.collect();
		let allocated = Default::default();
		Self {
			job,
			tasks,
			allocated,
		}
	}
	/// Allocate tasks from the job
	///
	/// This function will not wait for tasks to be available.
	///
	/// The returned object contains all info the client needs to start processing
	pub(super) async fn allocate(&self) -> Option<(Uuid, Arc<AllocatedTask>)> {
		let allocated = self.tasks.first().and_then(ScheduledTaskInfo::allocate);
		match allocated {
			None => None,
			Some(allocated) => {
				let id = Uuid::new_v4();
				let allocated = Arc::new(allocated);
				self.allocated
					.write()
					.await
					.insert(id, Arc::downgrade(&allocated));
				Some((id, allocated))
			}
		}
	}
	/// Get allocated task from its id
	///
	/// While the task is allocated, JobScheduler will keep a reference to it and its id.
	/// This allows other parts to get access to the allocated task
	pub(crate) async fn get_allocated(&self, uuid: &Uuid) -> Option<Arc<AllocatedTask>> {
		self.allocated
			.read()
			.await
			.get(uuid)
			.map(Weak::upgrade)
			.unwrap_or_default()
	}
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use uuid::Uuid;

	use crate::job_manager::job_scheduler::JobScheduler;
	use crate::jobs::Job;

	#[tokio::test]
	async fn do_not_segment_job_allocate_return_some() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		let allocated = scheduler.allocate().await;

		assert!(allocated.is_some());
	}

	#[tokio::test]
	async fn allocated_task_has_id() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		let (id, _allocated): (Uuid, _) = scheduler.allocate().await.expect("Should allocate");

		assert!(!id.is_nil());
	}

	#[tokio::test]
	async fn access_allocated_task_from_id() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		let (id, allocated): (Uuid, _) = scheduler.allocate().await.expect("Should allocate");

		let got = scheduler.get_allocated(&id).await.expect("Should find");
		assert!(Arc::ptr_eq(&got, &allocated));
	}

	#[tokio::test]
	async fn access_allocated_with_invalid_id_returns_none() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);

		let uuid = Uuid::from_u64_pair(123, 123);
		let got = scheduler.get_allocated(&uuid).await;
		assert!(got.is_none());
	}

	#[tokio::test]
	async fn do_not_segment_job_allocate_return_only_once() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		let _allocated = scheduler.allocate().await;
		let allocated = scheduler.allocate().await;

		assert!(allocated.is_none());
	}

	#[tokio::test]
	async fn allocated_job_will_be_available_again_after_allocated_destruction() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		{
			let allocated = scheduler.allocate().await;
			let none_allocated = scheduler.allocate().await;
			assert!(
				allocated.is_some() && none_allocated.is_none(),
				"Second should not be allocated"
			);
		}
		let allocated = scheduler.allocate().await;
		assert!(
			allocated.is_some(),
			"Should be allocated after first get destructed"
		);
	}

	#[tokio::test]
	async fn do_not_segment_job_allocatd_has_same_job_parameters() {
		let job: Arc<_> = Job::fake().into();
		let scheduler = JobScheduler::new(job.clone());
		let (_, allocated) = scheduler.allocate().await.expect("Should be available");
		assert_eq!(allocated.as_task().parameters, job.parameters);
	}
}
