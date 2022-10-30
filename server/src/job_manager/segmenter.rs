//! Module that manages tasks from jobs, should handle if task fails, and when job is complete.
//! Also handle status tracking
use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::job_manager::segmenter::job_segmenter::{JobSegmenter, SegmentAllocation};
use crate::jobs::{Job, JobParams, Task};

///Stores all the data needed to create a [Task], except the id.
#[derive(Clone)]
struct Segment {
	input_path: String,
	parameters: JobParams,
}

impl Segment {
	pub(crate) fn into_task(self, job_id: &Uuid, task_id: &Uuid) -> Task {
		Task {
			job_id: *job_id,
			id: *task_id,
			input_path: self.input_path,
			parameters: self.parameters,
		}
	}
}

mod job_segmenter {
	use std::sync::atomic::{AtomicU8, Ordering};
	use std::sync::Arc;

	use tokio::sync::OnceCell;
	use uuid::Uuid;

	use crate::job_manager::segmenter::Segment;
	use crate::jobs::Job;

	pub(super) struct JobSegmenter {
		job: Arc<Job>,
		job_id: Uuid,
		segments: OnceCell<SegmentEntry>,
	}

	type SegmentData = Arc<(Segment, AtomicU8, Uuid)>;

	///Hold segment data
	struct SegmentEntry(SegmentData);

	///RAII wrapper for Segment allocation
	pub(super) struct SegmentAllocation(SegmentData);

	enum AllocationState {
		Queue = 0,
		Allocated = 1,
		Completed = 2,
	}

	impl SegmentAllocation {
		pub fn as_segment(&self) -> &Segment {
			&self.0 .0
		}
		pub fn job_id(&self) -> &Uuid {
			&self.0 .2
		}
		pub fn set_completed(&self) -> usize {
			let state = &self.0 .1;
			state.store(AllocationState::Completed as u8, Ordering::Release);
			0
		}
	}

	impl Drop for SegmentAllocation {
		fn drop(&mut self) {
			let _ = self.0 .1.compare_exchange(
				AllocationState::Allocated as u8,
				AllocationState::Queue as u8,
				Ordering::SeqCst,
				Ordering::Acquire,
			);
		}
	}

	impl SegmentEntry {
		fn allocate(&self) -> Option<SegmentAllocation> {
			let bool = &self.0 .1;
			let allocate = bool.compare_exchange(
				AllocationState::Queue as u8,
				AllocationState::Allocated as u8,
				Ordering::SeqCst,
				Ordering::Acquire,
			);
			allocate.map(|_| SegmentAllocation(self.0.clone())).ok()
		}
	}

	impl JobSegmenter {
		pub fn new(job: Arc<Job>, job_id: Uuid) -> Self {
			Self {
				job,
				job_id,
				segments: OnceCell::new(),
			}
		}
		pub fn get_available(&self) -> Option<SegmentAllocation> {
			self.segments
				.get()
				.or_else(|| self.next_segment())
				.and_then(|segment| segment.allocate())
		}
	}

	impl JobSegmenter {
		///Internal function to segment jobs.
		///
		///This may differ for different kinds of segmentation.
		fn next_segment(&self) -> Option<&SegmentEntry> {
			let segment = Segment {
				input_path: format!("/api/jobs/{}/source", self.job_id),
				parameters: self.job.parameters.clone(),
			};
			let segment = Arc::new((
				segment,
				AtomicU8::new(AllocationState::Queue as u8),
				self.job_id,
			));
			self.segments
				.set(SegmentEntry(segment))
				.ok()
				.and_then(|()| self.segments.get())
		}
	}

	#[cfg(test)]
	mod test {
		use std::sync::Arc;

		use uuid::Uuid;

		use crate::job_manager::segmenter::job_segmenter::JobSegmenter;
		use crate::jobs::{Job, JobParams, Source};
		use crate::storage::FileRef;

		#[test]
		fn job_segmenter_get_available_return_some_for_do_not_segment() {
			let source = Source::File(FileRef::fake());
			let parameters = JobParams::sample_params();
			let job_uuid = Uuid::new_v4();
			let job = Arc::new(Job::new(source, parameters));
			let segmenter = JobSegmenter::new(job, job_uuid);

			let available = segmenter.get_available();
			assert!(available.is_some())
		}

		#[test]
		fn job_segmenter_get_available_twice_return_none() {
			let source = Source::File(FileRef::fake());
			let parameters = JobParams::sample_params();
			let job_uuid = Uuid::new_v4();
			let job = Arc::new(Job::new(source, parameters));
			let segmenter = JobSegmenter::new(job, job_uuid);

			let _available = segmenter.get_available();
			let available = segmenter.get_available();
			assert!(available.is_none())
		}

		#[test]
		fn job_segmenter_after_drop_can_allocate_again() {
			let source = Source::File(FileRef::fake());
			let parameters = JobParams::sample_params();
			let job_uuid = Uuid::new_v4();
			let job = Arc::new(Job::new(source, parameters));
			let segmenter = JobSegmenter::new(job, job_uuid);

			let available = segmenter.get_available();
			drop(available);
			let available = segmenter.get_available();
			assert!(available.is_some())
		}
	}
}

pub(super) struct TaskScheduler {
	allocated: RwLock<Option<(SegmentAllocation, Uuid)>>,
	segmenter: JobSegmenter,
}

impl TaskScheduler {
	///Interface to allocate tasks.
	///
	///Only allocate if task is available, wont wait until new task is available.
	///
	///The returned task will be marked as running.
	pub(super) async fn allocate(&self) -> Option<Task> {
		let mut allocated = self.allocated.write().await;
		let task = self
			.segmenter
			.get_available()
			.and_then(|segment| {
				*allocated = Some((segment, Uuid::new_v4()));
				allocated
					.as_ref()
					.map(|(seg, id)| (seg.as_segment().clone(), seg.job_id(), id))
			})
			.map(|(segment, job_id, id)| segment.into_task(job_id, id));
		task
	}

	pub(crate) async fn get_task(&self, id: &Uuid) -> Option<Task> {
		self.allocated
			.read()
			.await
			.as_ref()
			.filter(|(_, task_id)| task_id == id)
			.map(|(segment, _)| {
				segment
					.as_segment()
					.clone()
					.into_task(segment.job_id(), &id)
			})
	}

	pub(crate) async fn cancel_task(&self, id: &Uuid) -> Result<(), ()> {
		let mut allocated = self.allocated.write().await;
		let should_remove = allocated
			.as_ref()
			.and_then(|(_, task_id)| (task_id == id).then_some(()));
		if should_remove.is_some() {
			*allocated = None;
		}
		should_remove.ok_or(())
	}

	pub(crate) async fn set_task_as_completed(&self, task_id: &Uuid) -> Option<usize> {
		let mut lock = self.allocated.write().await;
		let should_remove = lock
			.as_ref()
			.and_then(|(_, id)| (id == task_id).then_some(()));
		should_remove
			.and_then(|()| lock.take())
			.map(|(allocated, _)| allocated.set_completed())
	}
}

impl Job {
	pub(super) fn make_segmenter(self: Arc<Self>, uuid: Uuid) -> TaskScheduler {
		TaskScheduler {
			allocated: RwLock::new(None), //While we only have one task, and don't restart
			segmenter: JobSegmenter::new(self, uuid),
		}
	}
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use uuid::Uuid;

	use crate::job_manager::segmenter::TaskScheduler;
	use crate::jobs::{Job, JobParams, Source};
	use crate::storage::FileRef;

	#[tokio::test]
	async fn segmenter_allocate_task_for_do_not_segment() {
		let segmenter = new_job_segmenter_with_single_task();

		let allocated = segmenter.allocate().await;
		assert!(allocated.is_some());
	}

	#[tokio::test]
	async fn segmenter_allocate_task_dont_segment_returns_none_second_time() {
		let segmenter = new_job_segmenter_with_single_task();

		segmenter.allocate().await;
		let task = segmenter.allocate().await;
		assert!(task.is_none());
	}

	#[tokio::test]
	async fn after_failed_allocate_can_get_previous_task() {
		let segmenter = new_job_segmenter_with_single_task();

		let task_id = segmenter.allocate().await.unwrap().id;
		segmenter.allocate().await;
		let got_task = segmenter.get_task(&task_id).await;
		assert!(got_task.is_some());
	}

	#[tokio::test]
	async fn segmenter_allocate_task_has_same_parameters() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.clone().make_segmenter(job_uuid);

		let task = segmenter.allocate().await.unwrap();
		assert_eq!(task.parameters, job.parameters);
	}

	#[tokio::test]
	async fn segmenter_allocate_task_not_segmented_has_source_as_input() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job_with_id = job.make_segmenter(job_uuid);

		let task = job_with_id.allocate().await.unwrap();
		let expected_path = format!("/api/jobs/{job_uuid}/source");
		let path = task.input_path;
		assert_eq!(
			path, expected_path,
			"Path should match /api/jobs/{{job_id}}/source"
		);
	}

	#[tokio::test]
	async fn generated_task_has_non_null_id() {
		let segmenter = new_job_segmenter_with_single_task();

		let task = segmenter.allocate().await.unwrap();
		let task_id = task.id;
		assert!(!task_id.is_nil())
	}

	#[tokio::test]
	async fn generated_task_has_job_id() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let task = segmenter.allocate().await.unwrap();
		let got_job_id = task.job_id;
		assert_eq!(got_job_id, job_uuid)
	}

	#[tokio::test]
	async fn get_task_returns_none_invalid_id() {
		let segmenter = new_job_segmenter_with_single_task();

		let _task = segmenter.allocate().await.unwrap();
		let uuid = Uuid::new_v4();
		let got_task = segmenter.get_task(&uuid).await;
		assert!(got_task.is_none())
	}

	#[tokio::test]
	async fn get_task_returns_equals_allocate() {
		let segmenter = new_job_segmenter_with_single_task();

		let task = segmenter.allocate().await.unwrap();
		let task_id = task.id;
		let got_task = segmenter.get_task(&task_id).await.unwrap();
		assert_eq!(got_task, task)
	}

	#[tokio::test]
	async fn cancel_task_with_valid_id_returns_ok() {
		let segmenter = new_job_segmenter_with_single_task();

		let task_id = segmenter.allocate().await.unwrap().id;
		let result = segmenter.cancel_task(&task_id).await;
		assert!(result.is_ok())
	}

	#[tokio::test]
	async fn after_cancel_can_not_get_canceled_task() {
		let segmenter = new_job_segmenter_with_single_task();

		let task_id = segmenter.allocate().await.unwrap().id;
		segmenter
			.cancel_task(&task_id)
			.await
			.expect("Should cancel");
		let task = segmenter.get_task(&task_id).await;
		assert!(task.is_none())
	}

	#[tokio::test]
	async fn cancel_task_with_invalid_id_returns_err() {
		let segmenter = new_job_segmenter_with_single_task();

		let _task_id = segmenter.allocate().await.unwrap().id;
		let other_id = Uuid::new_v4();
		let result = segmenter.cancel_task(&other_id).await;
		assert!(result.is_err())
	}

	#[tokio::test]
	async fn after_cancel_can_allocate_again() {
		let segmenter = new_job_segmenter_with_single_task();

		let task_id = segmenter.allocate().await.unwrap().id;
		segmenter.cancel_task(&task_id).await.unwrap();
		let task = segmenter.allocate().await;
		assert!(task.is_some())
	}

	fn new_job_segmenter_with_single_task() -> TaskScheduler {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		job.make_segmenter(job_uuid)
	}

	#[tokio::test]
	async fn set_invalid_task_as_completed_returns_none() {
		let segmenter = new_job_segmenter_with_single_task();
		let _task = segmenter.allocate().await.unwrap();
		let invalid_id = Uuid::new_v4();

		let res = segmenter.set_task_as_completed(&invalid_id).await;
		assert!(res.is_none());
	}

	#[tokio::test]
	async fn set_task_as_completed_returns_segment_number() {
		let segmenter = new_job_segmenter_with_single_task();
		let task = segmenter.allocate().await.unwrap();

		let res: Option<usize> = segmenter.set_task_as_completed(&task.id).await;
		assert!(res.is_some())
	}

	#[tokio::test]
	async fn after_task_is_completed_still_can_not_allocate() {
		let segmenter = new_job_segmenter_with_single_task();
		let task = segmenter.allocate().await.unwrap();

		segmenter.set_task_as_completed(&task.id).await.unwrap();
		let task = segmenter.allocate().await;
		assert!(task.is_none(), "Should not allocate completed segment")
	}

	#[tokio::test]
	async fn after_task_is_completed_get_task_returns_none() {
		let segmenter = new_job_segmenter_with_single_task();
		let task = segmenter.allocate().await.unwrap();

		segmenter.set_task_as_completed(&task.id).await.unwrap();
		let got = segmenter.get_task(&task.id).await;
		assert!(got.is_none())
	}
}
