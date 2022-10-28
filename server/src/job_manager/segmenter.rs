//! Module that manages tasks from jobs, should handle if task fails, and when job is complete.
//! Also handle status tracking
use std::sync::{Arc, Weak};

use tokio::sync::{OnceCell, RwLock};
use uuid::Uuid;

use crate::jobs::{Job, JobParams, Task};

///Stores all the data needed to create a [Task], except the id.
#[derive(Clone)]
struct Segment {
	input_path: String,
	parameters: JobParams,
}

impl Segment {
	pub(crate) fn into_task(self, task_id: &Uuid) -> Task {
		Task {
			id: *task_id,
			input_path: self.input_path,
			parameters: self.parameters,
		}
	}
}

pub(super) struct JobSegmenter {
	job: Weak<Job>,
	job_id: Uuid,
	allocated: RwLock<Option<Task>>,
	segment: OnceCell<Segment>,
}

impl JobSegmenter {
	///Interface to allocate tasks.
	///
	///Only allocate if task is available, wont wait until new task is available.
	///
	///The returned task will be marked as running.
	pub(super) async fn allocate(&self) -> Option<Task> {
		let mut allocated = self.allocated.write().await;
		if allocated.is_some() {
			return None;
		}
		let task = self
			.next_segment()
			.map(|segment| segment.into_task(&Uuid::new_v4()));
		*allocated = task.clone();
		task
	}

	pub(crate) async fn get_task(&self, id: &Uuid) -> Option<Task> {
		self.allocated
			.read()
			.await
			.as_ref()
			.filter(|task| id == &task.id)
			.cloned()
	}

	pub(crate) async fn cancel_task(&self, id: &Uuid) -> bool {
		let mut allocated = self.allocated.write().await;
		let should_remove = allocated
			.as_ref()
			.map(|task| &task.id == id)
			.unwrap_or_default();
		if should_remove {
			*allocated = None;
		}
		should_remove
	}
}

impl Job {
	pub(super) fn make_segmenter(self: &Arc<Self>, uuid: Uuid) -> JobSegmenter {
		JobSegmenter {
			job: Arc::downgrade(self),
			job_id: uuid,
			allocated: RwLock::new(None), //While we only have one task, and don't restart
			segment: OnceCell::new(),
		}
	}
}

impl JobSegmenter {
	///Internal function to segment jobs.
	///
	///This may differ for different kinds of segmentation.
	fn next_segment(&self) -> Option<Segment> {
		self.job
			.upgrade()
			.map(|upgraded| Segment {
				input_path: format!("/api/jobs/{}/source", self.job_id),
				parameters: upgraded.parameters.clone(),
			})
			.and_then(|segment| {
				let res = self.segment.set(segment.clone());
				res.ok().and(Some(segment))
			})
	}
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use uuid::Uuid;

	use crate::jobs::{Job, JobParams, Source};

	#[tokio::test]
	async fn segmenter_allocate_task_for_do_not_segment() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job = job.make_segmenter(job_uuid);

		let allocated = job.allocate().await;
		assert!(allocated.is_some());
	}

	#[tokio::test]
	async fn segmenter_allocate_task_dont_segment_returns_none_second_time() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job = job.make_segmenter(job_uuid);

		job.allocate().await;
		let task = job.allocate().await;
		assert!(task.is_none());
	}

	#[tokio::test]
	async fn after_failed_allocate_can_get_previous_task() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job = job.make_segmenter(job_uuid);

		let task_id = job.allocate().await.unwrap().id;
		job.allocate().await;
		let got_task = job.get_task(&task_id).await;
		assert!(got_task.is_some());
	}

	#[tokio::test]
	async fn segmenter_allocate_task_has_same_parameters() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job_with_id = job.make_segmenter(job_uuid);

		let task = job_with_id.allocate().await.unwrap();
		assert_eq!(task.parameters, job.parameters);
	}

	#[tokio::test]
	async fn segmenter_allocate_task_not_segmented_has_source_as_input() {
		let source = Source::Local(Uuid::new_v4());
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
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let task = segmenter.allocate().await.unwrap();
		let task_id = task.id;
		assert!(!task_id.is_nil())
	}

	#[tokio::test]
	async fn get_task_returns_none_invalid_id() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let _task = segmenter.allocate().await.unwrap();
		let uuid = Uuid::new_v4();
		let got_task = segmenter.get_task(&uuid).await;
		assert!(got_task.is_none())
	}

	#[tokio::test]
	async fn get_task_returns_equals_allocate() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let task = segmenter.allocate().await.unwrap();
		let task_id = task.id;
		let got_task = segmenter.get_task(&task_id).await.unwrap();
		assert_eq!(got_task, task)
	}

	#[tokio::test]
	async fn cancel_task_with_valid_id_returns_true() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let task_id = segmenter.allocate().await.unwrap().id;
		let result = segmenter.cancel_task(&task_id).await;
		assert!(result)
	}

	#[tokio::test]
	async fn after_cancel_can_not_get_canceled_task() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let task_id = segmenter.allocate().await.unwrap().id;
		segmenter.cancel_task(&task_id).await;
		let task = segmenter.get_task(&task_id).await;
		assert!(task.is_none())
	}

	#[tokio::test]
	async fn cancel_task_with_invalid_id_returns_false() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let _task_id = segmenter.allocate().await.unwrap().id;
		let other_id = Uuid::new_v4();
		let result = segmenter.cancel_task(&other_id).await;
		assert!(!result)
	}
}
