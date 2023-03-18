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
	fn segment(job: &Job) -> TaskInfo {
		match job.segmenter {
			Segmenter::DoNotSegment => TaskInfo {
				input: job.source.clone(),
				parameters: job.parameters.clone(),
			},
		}
	}
}

struct TaskInfo {
	input: Source,
	parameters: JobParams,
}

#[cfg(test)]
mod test {
	use crate::jobs::{Job, Segmenter};

	#[test]
	fn don_not_segment_generate_task_info_with_same_input_and_parameters() {
		let job = Job::fake();
		let segmented = Segmenter::segment(&job);
		assert_eq!(segmented.parameters, job.parameters);
		assert_eq!(segmented.input, job.source);
	}
}
