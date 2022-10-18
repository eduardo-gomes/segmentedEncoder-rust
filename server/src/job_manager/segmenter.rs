//! Module that manages tasks from jobs, should handle if task fails, and when job is complete.
//! Also handle status tracking
use uuid::Uuid;

use crate::jobs::{Job, Task};

pub trait JobSegmenter {
	fn next_task(&self) -> Option<Task>;
}

pub struct JobWithId<'a>(&'a Uuid, &'a Job);

impl Job {
	fn with_id<'a>(&'a self, uuid: &'a Uuid) -> JobWithId<'a> {
		JobWithId(uuid, self)
	}
}

impl<'a> JobSegmenter for JobWithId<'a> {
	fn next_task(&self) -> Option<Task> {
		let uuid = &self.0;
		Some(Task {
			input: format!("/api/jobs/{uuid}/source"),
			parameters: self.1.parameters.clone(),
		})
	}
}

#[cfg(test)]
mod test {
	use uuid::Uuid;

	use crate::job_manager::segmenter::JobSegmenter;
	use crate::jobs::{Job, JobParams, Segmenter, Source};

	#[test]
	fn job_next_task_on_dont_segment_returns_single_task() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let segmenter = Segmenter::DoNotSegment;
		let job_uuid = Uuid::new_v4();
		let job = Job::new(source, parameters, segmenter);
		let job = job.with_id(&job_uuid);

		let task = job.next_task();
		assert!(task.is_some());
	}

	#[test]
	fn job_next_task_has_same_parameters() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let segmenter = Segmenter::DoNotSegment;
		let job_uuid = Uuid::new_v4();
		let job = Job::new(source, parameters, segmenter);
		let job_with_id = job.with_id(&job_uuid);

		let task = job_with_id.next_task().unwrap();
		assert_eq!(task.parameters, job.parameters);
	}

	#[test]
	fn job_task_not_segmented_has_source_as_input() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let segmenter = Segmenter::DoNotSegment;
		let job_uuid = Uuid::new_v4();
		let job = Job::new(source, parameters, segmenter);
		let job_with_id = job.with_id(&job_uuid);

		let task = job_with_id.next_task().unwrap();
		let expected_path = format!("/api/jobs/{job_uuid}/source");
		let path = task.input;
		assert_eq!(
			path, expected_path,
			"Path should match /api/jobs/{{job_id}}/source"
		);
	}
}
