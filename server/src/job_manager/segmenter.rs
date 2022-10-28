//! Module that manages tasks from jobs, should handle if task fails, and when job is complete.
//! Also handle status tracking
use std::sync::{Arc, Weak};

use tokio::sync::OnceCell;
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
	task_id: Uuid,
	segment: OnceCell<Segment>,
}

impl JobSegmenter {
	///Interface to allocate tasks.
	///
	///The returned task will be marked as running.
	pub(super) fn allocate(&self) -> Option<Task> {
		self.next_segment()
			.map(|segment| segment.into_task(&self.task_id))
	}

	pub(crate) fn get_task(&self, id: &Uuid) -> Option<Task> {
		self.segment
			.get()
			.filter(|_| id == &self.task_id)
			.cloned()
			.map(|segment| segment.into_task(id))
	}

	pub(crate) fn cancel_task(&self, id: &Uuid) -> bool {
		self.get_task(id).is_some()
	}
}

impl Job {
	pub(super) fn make_segmenter(self: &Arc<Self>, uuid: Uuid) -> JobSegmenter {
		JobSegmenter {
			job: Arc::downgrade(self),
			job_id: uuid,
			task_id: Uuid::new_v4(), //While we only have one task, and don't restart
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
	fn get_task_returns_equals_allocate() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let task = segmenter.allocate().unwrap();
		let task_id = task.id;
		let got_task = segmenter.get_task(&task_id).unwrap();
		assert_eq!(got_task, task)
	}

	#[test]
	fn cancel_task_with_valid_id_returns_true() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let task_id = segmenter.allocate().unwrap().id;
		let result = segmenter.cancel_task(&task_id);
		assert!(result)
	}

	#[test]
	fn cancel_task_with_invalid_id_returns_false() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let segmenter = job.make_segmenter(job_uuid);

		let _task_id = segmenter.allocate().unwrap().id;
		let other_id = Uuid::new_v4();
		let result = segmenter.cancel_task(&other_id);
		assert!(!result)
	}
}
