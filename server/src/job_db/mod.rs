//! # Job database module
//!
//! This module stores all jobs information, and parses jobs into the protobuf compatible struct.
//!
//! ## Operations
//!
//! Main operations:
//! - Create job
//! - Insert task to job
//! - Allocate task
//!
//! Secondary operations
//! - List jobs/tasks
//! - Delete job
//! - Restart task
//! - Timeout task

use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
trait JobDb<JOB, TASK> {
	async fn get_job(&self, id: &Uuid) -> Result<Option<JOB>, std::io::Error>;
	async fn create_job(&self, job: JOB) -> Result<Uuid, std::io::Error>;
}

mod local {
	use std::collections::HashMap;
	use std::io::Error;
	use std::sync::{Mutex, MutexGuard};

	use async_trait::async_trait;
	use uuid::Uuid;

	use crate::job_db::JobDb;

	#[derive(Default)]
	pub struct LocalJobDb<JOB: Sync + Send + Clone, TASK: Sync + Send> {
		jobs: Mutex<HashMap<Uuid, (JOB, HashMap<Uuid, TASK>)>>,
	}

	impl<JOB: Sync + Send + Clone, TASK: Sync + Send> LocalJobDb<JOB, TASK> {
		fn lock(&self) -> MutexGuard<'_, HashMap<Uuid, (JOB, HashMap<Uuid, TASK>)>> {
			self.jobs
				.lock()
				.unwrap_or_else(|poison| poison.into_inner())
		}
	}

	#[async_trait]
	impl<JOB: Sync + Send + Clone, TASK: Sync + Send> JobDb<JOB, TASK> for LocalJobDb<JOB, TASK> {
		async fn get_job(&self, id: &Uuid) -> Result<Option<JOB>, Error> {
			let job = self.lock().get(id).map(|(job, _)| job).cloned();
			Ok(job)
		}

		async fn create_job(&self, job: JOB) -> Result<Uuid, Error> {
			let key = Uuid::new_v4();
			self.lock().insert(key, (job, Default::default()));
			Ok(key)
		}
	}

	#[cfg(test)]
	mod test {
		use uuid::Uuid;

		use crate::job_db::local::LocalJobDb;
		use crate::job_db::JobDb;

		#[tokio::test]
		async fn get_nonexistent_job_none() {
			let manager = LocalJobDb::<(), ()>::default();
			let res = manager.get_job(&Uuid::from_u64_pair(1, 1)).await.unwrap();
			assert!(res.is_none())
		}

		#[tokio::test]
		async fn get_job_after_create() {
			let manager = LocalJobDb::<String, ()>::default();
			let job = "Job 1".to_string();
			let id = manager.create_job(job.clone()).await.unwrap();
			let res = manager.get_job(&id).await.unwrap().unwrap();
			assert_eq!(res, job)
		}
	}
}
