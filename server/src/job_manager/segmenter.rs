//! Module that manages tasks from jobs, should handle if task fails, and when job is complete.
//! Also handle status tracking
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use uuid::Uuid;

use crate::jobs::{Job, Task};

pub(super) struct JobSegmenter {
	job: Weak<Job>,
	job_id: Uuid,
	task_id: Uuid,
	generated: AtomicBool,
}

impl JobSegmenter {
	pub(crate) fn get_task(&self, id: &Uuid) -> Option<()> {
		match self.generated.load(Ordering::Relaxed) {
			true => match id == &self.task_id {
				true => Some(()),
				false => None,
			},
			false => None,
		}
	}
}

impl JobSegmenter {
	///Interface to allocate tasks.
	///
	///The returned task will be marked as running.
	pub(super) fn allocate(&self) -> Option<Task> {
		self.next_task()
	}
}

impl Job {
	pub(super) fn make_segmenter(self: &Arc<Self>, uuid: Uuid) -> JobSegmenter {
		JobSegmenter {
			job: Arc::downgrade(self),
			job_id: uuid,
			task_id: Uuid::new_v4(), //While we only have one task, and don't restart
			generated: AtomicBool::from(false),
		}
	}
}

impl JobSegmenter {
	///Internal function to segment tasks.
	///
	///This may differ for different kinds of segmentation.
	fn next_task(&self) -> Option<Task> {
		let uuid = self.job_id;
		if self
			.generated
			.compare_exchange(false, true, Ordering::SeqCst, Ordering::Acquire)
			.is_err()
		{
			return None;
		}
		self.job.upgrade().map(|upgraded| Task {
			id: self.task_id,
			input_path: format!("/api/jobs/{uuid}/source"),
			parameters: upgraded.parameters.clone(),
		})
	}
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use uuid::Uuid;

	use crate::jobs::{Job, JobParams, Source};

	#[test]
	fn segmenter_allocate_task_for_do_not_segment() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job = job.make_segmenter(job_uuid);

		let allocated = job.allocate();
		assert!(allocated.is_some());
	}

	#[test]
	fn segmenter_allocate_task_dont_segment_returns_none_second_time() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job = job.make_segmenter(job_uuid);

		job.allocate();
		let task = job.allocate();
		assert!(task.is_none());
	}

	#[test]
	fn segmenter_allocate_task_has_same_parameters() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job_with_id = job.make_segmenter(job_uuid);

		let task = job_with_id.allocate().unwrap();
		assert_eq!(task.parameters, job.parameters);
	}

	#[test]
	fn segmenter_allocate_task_not_segmented_has_source_as_input() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job_with_id = job.make_segmenter(job_uuid);

		let task = job_with_id.allocate().unwrap();
		let expected_path = format!("/api/jobs/{job_uuid}/source");
		let path = task.input_path;
		assert_eq!(
			path, expected_path,
			"Path should match /api/jobs/{{job_id}}/source"
		);
	}

	#[test]
	fn generated_task_has_non_null_id() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let task = segmenter.allocate().unwrap();
		let task_id = task.id;
		assert!(!task_id.is_nil())
	}

	#[test]
	fn get_task_returns_none_invalid_id() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let _task = segmenter.allocate().unwrap();
		let uuid = Uuid::new_v4();
		let got_task = segmenter.get_task(&uuid);
		assert!(got_task.is_none())
	}

	#[test]
	fn get_task_returns_some_with_valid_id() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let task = segmenter.allocate().unwrap();
		let task_id = task.id;
		let got_task = segmenter.get_task(&task_id);
		assert!(got_task.is_some())
	}
}
