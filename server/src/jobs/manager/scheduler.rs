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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::{OnceCell, SetError};
use uuid::Uuid;

pub(crate) use allocator::WeakMapEntryArc;

use crate::jobs::manager::scheduler::allocator::{WeakUuidMap, WeakUuidMapRandomKey};
use crate::jobs::segmenter::TaskInfo;
use crate::jobs::{Job, Segmenter};
use crate::storage::FileRef;

mod allocator;

#[derive(Debug)] //Derive debug for temporary log
///A ScheduledTask
///
///Allows to allocate the task, holds the [TaskInfo], and stores the output
struct ScheduledTaskInfo {
	///Atomic flag used to mark allocation and if it is available to be allocated
	available: AtomicBool,
	task: TaskInfo,
	output: OnceCell<FileRef>,
}

impl ScheduledTaskInfo {
	///Check if task is available, and return an [AllocatedTask] tracking this task allocation
	fn allocate(self: &Arc<Self>) -> Option<AllocatedTask> {
		let available = self.available.swap(false, Ordering::AcqRel);
		match available {
			true => Some(AllocatedTask {
				scheduled: self.clone(),
			}),
			false => None,
		}
	}
}

#[derive(Debug)] //Derive debug for temporary log
pub(crate) struct JobScheduler {
	job: Arc<Job>,
	tasks: Vec<Arc<ScheduledTaskInfo>>,
	allocated: Arc<WeakUuidMap<AllocatedTask>>,
}

/// Reference counting pointer to [AllocatedTask] inside a [WeakUuidMap]
pub type AllocatedTaskRef = WeakMapEntryArc<AllocatedTask>;

#[derive(Debug)] //Derive debug for temporary log
///Marks an allocated task
///
/// This object keeps the task allocated until it is dropped.
/// Also allow access to the allocated task
pub struct AllocatedTask {
	scheduled: Arc<ScheduledTaskInfo>,
}

impl AllocatedTask {
	pub(crate) fn as_task(&self) -> &TaskInfo {
		&self.scheduled.task
	}
	///Set the task output, only works once. After the output is set it will always fail
	pub(crate) fn set_output(&self, output: FileRef) -> Result<(), FileRef> {
		self.scheduled.output.set(output).map_err(|err| match err {
			SetError::AlreadyInitializedError(e) => e,
			SetError::InitializingError(e) => e,
		})
	}
	pub(crate) fn get_output(&self) -> Option<&FileRef> {
		self.scheduled.output.get()
	}
}

impl Drop for AllocatedTask {
	fn drop(&mut self) {
		if !self.scheduled.output.initialized() {
			self.scheduled.available.store(true, Ordering::Release);
		}
	}
}

impl JobScheduler {
	pub(super) fn new(job: Arc<Job>) -> Self {
		let tasks = Segmenter::segment(job.as_ref())
			.tasks
			.into_iter()
			.map(|info| {
				ScheduledTaskInfo {
					available: true.into(),
					task: info,
					output: Default::default(),
				}
				.into()
			})
			.collect();
		let allocated = WeakUuidMap::new();
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
	pub(super) async fn allocate(&self) -> Option<(Uuid, AllocatedTaskRef)> {
		let allocated = self.tasks.first().and_then(ScheduledTaskInfo::allocate);
		match allocated {
			None => None,
			Some(allocated) => {
				let (id, arc) = self.allocated.insert_random_key(allocated).await;
				Some((id, arc))
			}
		}
	}
	/// Get allocated task from its id
	///
	/// While the task is allocated, JobScheduler will keep a reference to it and its id.
	/// This allows other parts to get access to the allocated task
	pub(crate) async fn get_allocated(&self, uuid: &Uuid) -> Option<AllocatedTaskRef> {
		self.allocated.get(uuid).await
	}

	pub async fn allocated_count(&self) -> usize {
		self.allocated.len().await
	}
	pub fn get_job(&self) -> &Arc<Job> {
		&self.job
	}
}

#[cfg(test)]
mod test {
	use std::ptr;
	use std::sync::Arc;

	use uuid::Uuid;

	use crate::jobs::manager::scheduler::JobScheduler;
	use crate::jobs::Job;
	use crate::storage::FileRef;

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
		assert!(ptr::eq(&*got, &*allocated));
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
	async fn do_not_segment_job_allocated_has_same_job_parameters() {
		let job: Arc<_> = Job::fake().into();
		let scheduler = JobScheduler::new(job.clone());
		let (_, allocated) = scheduler.allocate().await.expect("Should be available");
		assert_eq!(allocated.as_task().parameters, job.parameters);
	}

	#[tokio::test]
	async fn new_scheduler_has_zero_allocated_tasks() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		assert_eq!(scheduler.allocated_count().await, 0);
	}

	#[tokio::test]
	async fn allocate_increments_allocated_count() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		let _allocated = scheduler.allocate().await.expect("Should allocate");

		assert_eq!(scheduler.allocated_count().await, 1);
	}

	#[tokio::test]
	async fn dropping_allocated_decrements_allocated_count() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		let allocated = scheduler.allocate().await.expect("Should allocate");
		assert_eq!(scheduler.allocated_count().await, 1);
		drop(allocated);
		assert_eq!(scheduler.allocated_count().await, 0);
	}

	#[tokio::test]
	async fn store_output_for_allocated() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		let (_id, task) = scheduler.allocate().await.expect("Should allocate");

		let output = FileRef::fake();

		assert!(
			task.set_output(output).is_ok(),
			"Should store, no concurrency"
		);
	}

	#[tokio::test]
	async fn get_output_before_set_returns_none() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		let (_id, task) = scheduler.allocate().await.expect("Should allocate");
		assert!(task.get_output().is_none());
	}

	#[tokio::test]
	async fn get_output_after_set() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		let (_id, task) = scheduler.allocate().await.expect("Should allocate");
		let file_ref = FileRef::fake();
		task.set_output(file_ref.clone())
			.expect("Should store, no concurrency");
		let got = task.get_output().expect("Should be available after set");
		assert_eq!(got, &file_ref);
	}

	#[tokio::test]
	async fn store_output_only_works_one_time() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		let (_id, task) = scheduler.allocate().await.expect("Should allocate");
		task.set_output(FileRef::fake())
			.expect("Should store, no concurrency");
		let result = task.set_output(FileRef::fake());
		assert!(result.is_err(), "Should not store again");
	}

	#[tokio::test]
	async fn drop_after_store_do_not_make_available() {
		let job = Job::fake().into();
		let scheduler = JobScheduler::new(job);
		let (_id, task) = scheduler.allocate().await.expect("Should allocate");
		task.set_output(FileRef::fake())
			.expect("Should store, no concurrency");
		drop(task);
		assert!(
			scheduler.allocate().await.is_none(),
			"Should not allocate again"
		);
	}
}
