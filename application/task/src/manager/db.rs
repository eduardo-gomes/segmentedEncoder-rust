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
//! - Update task status/set as finished
//!
//! Secondary operations
//! - List jobs/tasks
//! - Delete job
//! - List allocated jobs
//! - Cancel task allocation
//! - Timeout task allocation
//!
//! Those operations could be implemented with 3 tables
//! - Jobs:
//! 	At least job_id
//! - Tasks:
//! 	At least job_id and task_number
//! - TaskInstances:
//! 	At least job_id, task_number and instance_id
//! Additionally:
//! - TaskDependencies
//! 	job_id, task_number, dependency_task_number

use uuid::Uuid;

trait JobDb<JOB, TASK> {
	async fn get_job(&self, id: &Uuid) -> Result<Option<JOB>, std::io::Error>;
	async fn create_job(&self, job: JOB) -> Result<Uuid, std::io::Error>;
	/// Append task to job and return the task index
	async fn append_task(&self, job_id: &Uuid, task: TASK) -> Result<usize, std::io::Error>;
	async fn get_tasks(&self, job_id: &Uuid) -> Result<Vec<TASK>, std::io::Error>;
	async fn get_task(&self, job_id: &Uuid, task_idx: usize) -> Result<TASK, std::io::Error> {
		let task = self.get_tasks(job_id).await?.into_iter().nth(task_idx);
		task.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "index out of bound"))
	}

	async fn allocate_task(&self) -> Result<Option<Uuid>, std::io::Error>;
}

mod local {
	use std::collections::HashMap;
	use std::io::{Error, ErrorKind};
	use std::sync::{Mutex, MutexGuard};

	use uuid::Uuid;

	use super::JobDb;

	#[derive(Default)]
	pub struct LocalJobDb<JOB: Sync + Send + Clone, TASK: Sync + Send + Clone> {
		jobs: Mutex<HashMap<Uuid, (JOB, Vec<(TASK, Option<Uuid>)>)>>,
	}

	impl<JOB: Sync + Send + Clone, TASK: Sync + Send + Clone> LocalJobDb<JOB, TASK> {
		fn lock(&self) -> MutexGuard<'_, HashMap<Uuid, (JOB, Vec<(TASK, Option<Uuid>)>)>> {
			self.jobs
				.lock()
				.unwrap_or_else(|poison| poison.into_inner())
		}
	}

	impl<JOB: Sync + Send + Clone, TASK: Sync + Send + Clone> JobDb<JOB, TASK>
		for LocalJobDb<JOB, TASK>
	{
		async fn get_job(&self, id: &Uuid) -> Result<Option<JOB>, Error> {
			let job = self.lock().get(id).map(|(job, _)| job).cloned();
			Ok(job)
		}

		async fn create_job(&self, job: JOB) -> Result<Uuid, Error> {
			let key = Uuid::new_v4();
			self.lock().insert(key, (job, Default::default()));
			Ok(key)
		}

		async fn append_task(&self, job_id: &Uuid, task: TASK) -> Result<usize, Error> {
			let mut guard = self.lock();
			let job = match guard.get_mut(job_id).map(|(_, tasks)| tasks) {
				None => return Err(Error::new(ErrorKind::NotFound, "Job not found")),
				Some(tasks) => tasks,
			};
			let idx = job.len();
			job.push((task, None));
			Ok(idx)
		}

		async fn get_tasks(&self, job_id: &Uuid) -> Result<Vec<TASK>, Error> {
			self.lock()
				.get(job_id)
				.map(|(_, tasks)| tasks.iter().map(|(task, _)| task).cloned().collect())
				.ok_or_else(|| Error::new(ErrorKind::NotFound, "Job not found"))
		}

		async fn allocate_task(&self) -> Result<Option<Uuid>, Error> {
			let mut binding = self.lock();
			let available = binding
				.values_mut()
				.flat_map(|(_, jobs)| {
					jobs.iter_mut()
						.filter(|(_, allocation)| allocation.is_none())
				})
				.next();
			match available {
				None => Ok(None),
				Some(available) => {
					let id = Uuid::new_v4();
					available.1 = Some(id);
					Ok(Some(id))
				}
			}
		}
	}

	#[cfg(test)]
	mod test {
		use std::io::ErrorKind;

		use uuid::Uuid;

		use super::JobDb;
		use super::LocalJobDb;

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

		#[tokio::test]
		async fn add_task_to_nonexistent_job_error() {
			let manager = LocalJobDb::<String, String>::default();
			let task = "Task 1".to_string();
			let first_task = manager.append_task(&Uuid::from_u64_pair(1, 2), task).await;
			assert_eq!(first_task.unwrap_err().kind(), ErrorKind::NotFound)
		}

		#[tokio::test]
		async fn get_task_nonexistent_job_error() {
			let manager = LocalJobDb::<String, String>::default();
			let res = manager.get_task(&Uuid::from_u64_pair(1, 2), 0).await;
			assert_eq!(res.unwrap_err().kind(), ErrorKind::NotFound)
		}

		#[tokio::test]
		async fn add_get_task_by_id() {
			let manager = LocalJobDb::<String, String>::default();
			let task = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job.clone()).await.unwrap();
			let task_idx = manager.append_task(&job_id, task.clone()).await.unwrap();
			let res = manager.get_task(&job_id, task_idx).await.unwrap();
			assert_eq!(task, res)
		}

		#[tokio::test]
		async fn add_get_all_tasks_nonexistent_job() {
			let manager = LocalJobDb::<String, String>::default();
			let error = manager
				.get_tasks(&Uuid::from_u64_pair(1, 3))
				.await
				.unwrap_err();
			assert_eq!(error.kind(), ErrorKind::NotFound)
		}

		#[tokio::test]
		async fn add_get_all_tasks_of_job() {
			let manager = LocalJobDb::<String, String>::default();
			let job = "Job 1".to_string();
			let task_1 = "Task 1".to_string();
			let task_2 = "Task 2".to_string();
			let job_id = manager.create_job(job.clone()).await.unwrap();
			manager.append_task(&job_id, task_1.clone()).await.unwrap();
			manager.append_task(&job_id, task_2.clone()).await.unwrap();
			let tasks = manager.get_tasks(&job_id).await.unwrap();
			assert_eq!(tasks, [task_1, task_2])
		}

		#[tokio::test]
		async fn add_task_to_job_returns_sequencial_id() {
			let manager = LocalJobDb::<String, String>::default();
			let job = "Job 1".to_string();
			let task_1 = "Task 1".to_string();
			let task_2 = "Task 2".to_string();
			let id = manager.create_job(job.clone()).await.unwrap();
			let first_task = manager.append_task(&id, task_1).await.unwrap();
			let second_task = manager.append_task(&id, task_2).await.unwrap();
			assert!(
				second_task > first_task,
				"Second task should have a greater id than first task, {} > {}",
				second_task,
				first_task
			);
		}

		#[tokio::test]
		async fn allocate_task_without_any_available_returns_none() {
			let manager = LocalJobDb::<String, String>::default();
			let allocation = manager.allocate_task().await.unwrap();
			assert!(allocation.is_none())
		}

		#[tokio::test]
		async fn allocate_task_returns_task_and_the_run_id() {
			let manager = LocalJobDb::<String, String>::default();
			let task = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let _task_idx = manager.append_task(&job_id, task).await.unwrap();
			let allocation_id: Uuid = manager.allocate_task().await.unwrap().unwrap();
			assert!(!allocation_id.is_nil())
		}

		#[tokio::test]
		async fn allocate_more_than_available_return_none() {
			let manager = LocalJobDb::<String, String>::default();
			let task = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			manager.append_task(&job_id, task).await.unwrap();
			let _allocated = manager.allocate_task().await.unwrap();
			let none = manager.allocate_task().await.unwrap();
			assert!(none.is_none())
		}

		#[tokio::test]
		async fn allocate_two_tasks() {
			let manager = LocalJobDb::<String, String>::default();
			let task_1 = "Task 1".to_string();
			let task_2 = "Task 2".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			manager.append_task(&job_id, task_1).await.unwrap();
			manager.append_task(&job_id, task_2).await.unwrap();
			let allocated_1 = manager.allocate_task().await.unwrap();
			let allocated_2 = manager.allocate_task().await.unwrap();
			assert!(allocated_1.is_some());
			assert!(allocated_2.is_some());
		}
	}
}
