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

use uuid::Uuid;

use crate::jobs::segmenter::TaskInfo;
use crate::jobs::{Job, Source};
use crate::storage::FileRef;

pub(crate) struct JobScheduler {
	job: Arc<Job>,
	uuid: Uuid,
	allocated: AtomicBool,
}

pub struct AllocatedTask {
	task: TaskInfo,
}

impl JobScheduler {
	pub(super) fn new(job: Arc<Job>, uuid: Uuid) -> Self {
		Self {
			job,
			uuid,
			allocated: false.into(),
		}
	}
	/// Allocate tasks from the job
	///
	/// This function will not wait for tasks to be available.
	///
	/// The returned object contains all info the client needs to start processing
	pub(super) async fn allocate(&self) -> Option<AllocatedTask> {
		let old = self.allocated.swap(true, Ordering::AcqRel);
		if !old {
			let task_info = TaskInfo {
				input: Source::File(FileRef::from(Uuid::nil())),
				parameters: self.job.parameters.clone(),
			};
			Some(AllocatedTask { task: task_info })
		} else {
			None
		}
	}
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use uuid::Uuid;

	use crate::job_manager::job_scheduler::JobScheduler;
	use crate::jobs::Job;

	#[test]
	fn new_job_scheduler_stores_uuid_passed_to_constructor() {
		let job = Job::fake().into();
		let uuid = Uuid::new_v4();
		let scheduler = JobScheduler::new(job, uuid);

		assert_eq!(scheduler.uuid, uuid);
	}

	#[tokio::test]
	async fn do_not_segment_job_allocate_return_some() {
		let job = Job::fake().into();
		let uuid = Uuid::new_v4();
		let scheduler = JobScheduler::new(job, uuid);
		let allocated = scheduler.allocate().await;

		assert!(allocated.is_some());
	}

	#[tokio::test]
	async fn do_not_segment_job_allocate_return_only_once() {
		let job = Job::fake().into();
		let uuid = Uuid::new_v4();
		let scheduler = JobScheduler::new(job, uuid);
		let _allocated = scheduler.allocate().await;
		let allocated = scheduler.allocate().await;

		assert!(allocated.is_none());
	}

	#[tokio::test]
	async fn do_not_segment_job_allocatd_has_same_job_parameters() {
		let job: Arc<_> = Job::fake().into();
		let uuid = Uuid::new_v4();
		let scheduler = JobScheduler::new(job.clone(), uuid);
		let allocated = scheduler.allocate().await.expect("Should be available");
		assert_eq!(allocated.task.parameters, job.parameters);
	}
}
