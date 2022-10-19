//! Module that manages tasks from jobs, should handle if task fails, and when job is complete.
//! Also handle status tracking
use std::sync::{Arc, Weak};

use uuid::Uuid;

use crate::jobs::{Job, Task};

struct JobSegmenter {
	job: Weak<Job>,
	job_id: Uuid,
}

impl Job {
	fn make_segmenter(self: &Arc<Self>, uuid: Uuid) -> JobSegmenter {
		JobSegmenter {
			job: Arc::downgrade(self),
			job_id: uuid,
		}
	}
}

impl JobSegmenter {
	fn next_task(&self) -> Option<Task> {
		let uuid = self.job_id;
		if let Some(upgraded) = self.job.upgrade() {
			Some(Task {
				input: format!("/api/jobs/{uuid}/source"),
				parameters: upgraded.parameters.clone(),
			})
		} else {
			None //Job was dropped
		}
	}
}

#[cfg(test)]
mod test {
	use std::sync::Arc;

	use uuid::Uuid;

	use crate::jobs::{Job, JobParams, Segmenter, Source};

	#[test]
	fn job_next_task_on_dont_segment_returns_single_task() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job = job.make_segmenter(job_uuid);

		let task = job.next_task();
		assert!(task.is_some());
	}

	#[test]
	fn job_next_task_has_same_parameters() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job_with_id = job.make_segmenter(job_uuid);

		let task = job_with_id.next_task().unwrap();
		assert_eq!(task.parameters, job.parameters);
	}

	#[test]
	fn job_task_not_segmented_has_source_as_input() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job_uuid = Uuid::new_v4();
		let job = Arc::new(Job::new(source, parameters));
		let job_with_id = job.make_segmenter(job_uuid);

		let task = job_with_id.next_task().unwrap();
		let expected_path = format!("/api/jobs/{job_uuid}/source");
		let path = task.input;
		assert_eq!(
			path, expected_path,
			"Path should match /api/jobs/{{job_id}}/source"
		);
	}
}
