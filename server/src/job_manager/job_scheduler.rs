//!
//! [JobScheduler] will hold a job and all tasks from this job.
//!
//! When a scheduler is created, it will generate all tasks that do not need preprocessing to be
//! created and them will start in the pre-execution or execution phases
//!
//! Jobs will have 3 phases:
//! - **Pre-execution**: Useful to analyze input and generate more tasks to be executed
//! - **Execution**: Runs the encoding
//! - **Post-execution**: Useful to merge all the artifacts into a single file
//!

use std::sync::Arc;

use uuid::Uuid;

use crate::jobs::{Job, Task};

pub(crate) struct JobScheduler {
	job: Arc<Job>,
	uuid: Uuid,
}

impl JobScheduler {
	pub(super) fn new(job: Arc<Job>, uuid: Uuid) -> Self {
		Self { job, uuid }
	}
	/// Allocate tasks from the job
	///
	/// This function will not wait for tasks to be available.
	///
	/// The returned object contains all info the client needs to start processing
	pub(super) async fn allocate(self: &Arc<Self>) -> Option<Task> {
		None
	}
}
