//! Module that manages tasks from jobs, should handle if task fails, and when job is complete.
//! Also handle status tracking
use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::jobs::manager::old_task_scheduler::old_job_segmenter::{
	OldJobSegmenter, OldSegmentAllocation,
};
use crate::jobs::{Job, JobParams, Task};
use crate::storage::FileRef;

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

#[deprecated]
#[path = "old_job_segmenter.rs"]
mod old_job_segmenter;

pub(crate) struct OldTaskScheduler {
	allocated: RwLock<Option<(OldSegmentAllocation, Uuid)>>,
	segmenter: OldJobSegmenter,
}

impl OldTaskScheduler {
	///Interface to allocate tasks.
	///
	///Only allocate if task is available, won't wait until new task is available.
	///
	///The returned task will be marked as running.
	pub(super) async fn allocate(&self) -> Option<Task> {
		let mut allocated = self.allocated.write().await;
		self.segmenter
			.get_available()
			.and_then(|segment| {
				*allocated = Some((segment, Uuid::new_v4()));
				allocated
					.as_ref()
					.map(|(seg, id)| (seg.as_segment().clone(), seg.job_id(), id))
			})
			.map(|(segment, job_id, id)| segment.into_task(job_id, id))
	}

	pub(crate) async fn get_task(&self, id: &Uuid) -> Option<Task> {
		let segment_into_task = |(segment, _): &(OldSegmentAllocation, _)| {
			segment.as_segment().clone().into_task(segment.job_id(), id)
		};
		self.find_task_and_map(id, segment_into_task).await
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
			.map(|(allocated, _)| allocated.set_completed().segment_number())
	}

	pub(crate) fn get_segment_output(&self, segment_number: usize) -> Option<FileRef> {
		self.segmenter
			.get_segment(segment_number)
			.map(|segment| segment.get_output())
			.unwrap_or_default()
	}

	pub(crate) async fn set_task_output(
		&self,
		id: &Uuid,
		output: FileRef,
	) -> Option<Result<(), ()>> {
		let set_output =
			|(allocation, _): &(OldSegmentAllocation, _)| allocation.set_output(output);
		self.find_task_and_map(id, set_output).await
	}

	///Get the stored [OldSegmentAllocation] by task id and map the result using mapper
	async fn find_task_and_map<U, F>(&self, id: &Uuid, mapper: F) -> Option<U>
	where
		F: FnOnce(&(OldSegmentAllocation, Uuid)) -> U,
	{
		self.allocated
			.read()
			.await
			.as_ref()
			.filter(|(_, task_id)| task_id == id)
			.map(mapper)
	}
}

impl Job {
	#[deprecated]
	pub(super) fn make_segmenter_and_scheduler(self: Arc<Self>, uuid: Uuid) -> OldTaskScheduler {
		OldTaskScheduler {
			allocated: RwLock::new(None), //While we only have one task, and don't restart
			segmenter: OldJobSegmenter::new(self, uuid),
		}
	}
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use uuid::Uuid;

	use crate::jobs::manager::old_task_scheduler::OldTaskScheduler;
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
		let segmenter = job.clone().make_segmenter_and_scheduler(job_uuid);

		let task = segmenter.allocate().await.unwrap();
		assert_eq!(task.parameters, job.parameters);
	}

	#[tokio::test]
	async fn segmenter_allocate_task_not_segmented_has_source_as_input() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job_with_id = job.make_segmenter_and_scheduler(job_uuid);

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
		let segmenter = job.make_segmenter_and_scheduler(job_uuid);

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

	fn new_job_segmenter_with_single_task() -> OldTaskScheduler {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		job.make_segmenter_and_scheduler(job_uuid)
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
	async fn after_task_is_completed_get_task_returns_none() {
		let segmenter = new_job_segmenter_with_single_task();
		let task = segmenter.allocate().await.unwrap();

		segmenter.set_task_as_completed(&task.id).await.unwrap();
		let got = segmenter.get_task(&task.id).await;
		assert!(got.is_none())
	}

	#[tokio::test]
	async fn set_task_output() {
		let segmenter = new_job_segmenter_with_single_task();
		let task = segmenter.allocate().await.unwrap();
		let output = FileRef::fake();

		let res = segmenter.set_task_output(&task.id, output).await.unwrap();
		assert!(res.is_ok())
	}

	#[tokio::test]
	async fn set_task_output_with_invalid_task_id_returns_none() {
		let segmenter = new_job_segmenter_with_single_task();
		let _task = segmenter.allocate().await.unwrap();
		let output = FileRef::fake();
		let id = Uuid::new_v4();

		let res = segmenter.set_task_output(&id, output).await;
		assert!(res.is_none())
	}

	#[tokio::test]
	async fn set_task_output_second_time_fails() {
		let segmenter = new_job_segmenter_with_single_task();
		let task = segmenter.allocate().await.unwrap();
		let output = FileRef::fake();

		segmenter
			.set_task_output(&task.id, output.clone())
			.await
			.unwrap()
			.unwrap();
		let res = segmenter.set_task_output(&task.id, output).await.unwrap();
		assert!(res.is_err())
	}

	#[tokio::test]
	async fn get_segment_output_with_invalid_segment_returns_none() {
		let segmenter = new_job_segmenter_with_single_task();

		let output = segmenter.get_segment_output(10);
		assert!(output.is_none())
	}

	#[tokio::test]
	async fn get_segment_output_after_set_task_output_and_complete() {
		let segmenter = new_job_segmenter_with_single_task();
		let task = segmenter.allocate().await.unwrap();
		let output = FileRef::fake();

		segmenter
			.set_task_output(&task.id, output.clone())
			.await
			.unwrap()
			.unwrap();
		let number = segmenter.set_task_as_completed(&task.id).await.unwrap();
		let got_output = segmenter.get_segment_output(number).unwrap();
		assert_eq!(got_output, output)
	}
}
