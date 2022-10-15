use uuid::Uuid;

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct JobParams {
	pub(crate) video_encoder: String,
	pub(crate) video_args: Option<String>,
	pub(crate) audio_encoder: Option<String>,
	pub(crate) audio_args: Option<String>,
}

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) enum Source {
	Local(Uuid),
}

impl JobParams {
	#[cfg(test)]
	pub(crate) fn sample_params() -> Self {
		JobParams {
			video_encoder: "libsvtav1".to_string(),
			video_args: None,
			audio_encoder: None,
			audio_args: None,
		}
	}
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub(crate) struct Job {
	pub source: Source,
	pub parameters: JobParams,
}

impl Job {
	pub(crate) fn new(source: Source, parameters: JobParams) -> Self {
		Job { source, parameters }
	}
}

#[cfg(test)]
mod test {
	use uuid::Uuid;

	use crate::jobs::{Job, JobParams, Source};

	#[test]
	fn job_takes_source_and_parameters() {
		let source = Source::Local(Uuid::new_v4());
		let parameters = JobParams::sample_params();
		let job = Job::new(source.clone(), parameters.clone());

		assert_eq!(job.source, source);
		assert_eq!(job.parameters, parameters);
	}
}
