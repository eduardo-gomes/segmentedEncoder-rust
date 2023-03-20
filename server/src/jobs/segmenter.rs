//! Module to segment jobs
//!
//! [Segmenter] will generate tasks(TaskInfo) and subjobs for [JobScheduler]

use crate::jobs::{Job, JobParams, Source};

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum Segmenter {
	DoNotSegment,
}

impl Segmenter {
	/// Will generate TaskInfos and SubJobs.
	///
	/// TaskInfo can be turned into a [Task](super::Task), and SubJob into a [Job]
	fn segment(job: &Job) -> JobSegments {
		match job.segmenter {
			Segmenter::DoNotSegment => {
				let task = TaskInfo {
					input: job.source.clone(),
					parameters: job.parameters.clone(),
				};
				JobSegments { tasks: vec![task] }
			}
		}
	}
}

struct JobSegments {
	tasks: Vec<TaskInfo>,
}

struct TaskInfo {
	input: Source,
	parameters: JobParams,
}

#[cfg(test)]
mod test {
	use crate::jobs::{Job, Segmenter};

	#[test]
	fn don_not_segment_generate_response_with_an_array_with_a_single_task_info() {
		let job = Job::fake();
		let segmented = Segmenter::segment(&job);
		let array: Vec<_> = segmented.tasks;
		assert_eq!(array.len(), 1);
	}

	#[test]
	fn don_not_segment_generate_task_info_with_same_input_and_parameters() {
		let job = Job::fake();
		let segmented = Segmenter::segment(&job);
		let task = segmented.tasks.first().expect("Should generate some");
		assert_eq!(task.parameters, job.parameters);
		assert_eq!(task.input, job.source);
	}
}
