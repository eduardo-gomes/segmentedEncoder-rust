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
	/// Will generate [TaskInfo]s and SubJobs.
	///
	/// TaskInfo can be sent to a client, and SubJob can be turned into a [Job]
	pub fn segment(job: &Job) -> JobSegments {
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

pub(crate) struct JobSegments {
	pub(crate) tasks: Vec<TaskInfo>,
}

///Struct containing all data used by client to execute a task.
///
///It is able to tell the input and output files
#[derive(Clone, Debug)] //Derive debug for temporary log
pub(crate) struct TaskInfo {
	pub(crate) input: Source,
	pub(crate) parameters: JobParams,
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
