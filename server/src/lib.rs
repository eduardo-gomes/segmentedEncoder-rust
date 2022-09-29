use axum::Router;

pub mod web;

/// Temporary function to 'build' the service.
/// Will be replaced with a proper builder to set service proprieties.
pub fn make_service() -> Router {
	use crate::job_manager::JobManager;
	use std::sync::{Arc, RwLock};
	let manager = Arc::new(RwLock::new(JobManager::new()));
	web::make_service(manager)
}

#[allow(dead_code)]
mod storage;

mod jobs {
	#[derive(Clone, Debug)]
	#[cfg_attr(test, derive(PartialEq))]
	pub struct JobParams {
		pub(crate) video_encoder: String,
	}
	#[derive(Clone, Debug)]
	#[cfg_attr(test, derive(PartialEq))]
	pub(crate) enum Source {
		Local(),
	}

	impl JobParams {
		#[cfg(test)]
		pub(crate) fn sample_params() -> Self {
			JobParams {
				video_encoder: "libsvtav1".to_string(),
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
		use crate::jobs::{Job, JobParams, Source};

		#[test]
		fn job_takes_source_and_parameters() {
			let source = Source::Local();
			let parameters = JobParams::sample_params();
			let job = Job::new(source.clone(), parameters.clone());

			assert_eq!(job.source, source);
			assert_eq!(job.parameters, parameters);
		}
	}
}

#[allow(dead_code)]
mod job_manager;
