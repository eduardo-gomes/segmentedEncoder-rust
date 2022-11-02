use uuid::Uuid;

use crate::storage::FileRef;

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct JobParams {
	pub(crate) video_encoder: Option<String>,
	pub(crate) video_args: Option<String>,
	pub(crate) audio_encoder: Option<String>,
	pub(crate) audio_args: Option<String>,
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum Source {
	File(FileRef),
}

impl JobParams {
	#[cfg(test)]
	pub(crate) fn sample_params() -> Self {
		JobParams {
			video_encoder: Some("libsvtav1".to_string()),
			video_args: Some("-crf 30".to_string()),
			audio_encoder: Some("libopus".to_string()),
			audio_args: None,
		}
	}
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum Segmenter {
	DoNotSegment,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) struct Job {
	pub source: Source,
	pub parameters: JobParams,
	pub segmenter: Segmenter,
}

impl Job {
	pub(crate) fn new(source: Source, parameters: JobParams) -> Self {
		Job {
			source,
			parameters,
			segmenter: Segmenter::DoNotSegment,
		}
	}
}

///Struct containing all data used by client to execute a task.
///
///It is able to tell the input and output files
#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct Task {
	pub job_id: Uuid,
	pub id: Uuid,
	pub input_path: String,
	pub parameters: JobParams,
}

#[cfg(test)]
mod test {
	use crate::jobs::{Job, JobParams, Source};
	use crate::storage::FileRef;

	#[test]
	pub(crate) fn job_takes_source_and_parameters() {
		let source = Source::File(FileRef::fake());
		let parameters = JobParams::sample_params();
		let job = Job::new(source.clone(), parameters.clone());

		assert_eq!(job.source, source);
		assert_eq!(job.parameters, parameters);
	}
}
