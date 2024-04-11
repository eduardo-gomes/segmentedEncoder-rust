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

#[cfg_attr(test, mockall::automock)]
pub(crate) trait JobDb<JOB, TASK> {
	async fn get_job(&self, id: &Uuid) -> Result<Option<JOB>, std::io::Error>;
	async fn create_job(&self, job: JOB) -> Result<Uuid, std::io::Error>;
	/// Append task to job and return the task index
	async fn append_task(
		&self,
		job_id: &Uuid,
		task: TASK,
		dep: &[u32],
	) -> Result<u32, std::io::Error>;
	async fn get_tasks(&self, job_id: &Uuid) -> Result<Vec<TASK>, std::io::Error>;
	async fn get_task(&self, job_id: &Uuid, task_idx: u32) -> Result<TASK, std::io::Error> {
		let task = self
			.get_tasks(job_id)
			.await?
			.into_iter()
			.nth(task_idx as usize);
		task.ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "index out of bound"))
	}
	async fn get_allocated_task(
		&self,
		job_id: &Uuid,
		task_id: &Uuid,
	) -> Result<Option<(TASK, u32)>, std::io::Error>;

	async fn allocate_task(&self) -> Result<Option<(Uuid, Uuid)>, std::io::Error>;
	///Mark the task as finished, allowing tasks that depend on this task to run
	async fn fulfill(&self, job_id: &Uuid, task_idx: u32) -> Result<(), std::io::Error>;
	async fn get_task_status(
		&self,
		job_id: &Uuid,
		task_idx: u32,
	) -> Result<Option<()>, std::io::Error>;
}

pub(crate) mod local {
	use std::collections::{BTreeSet, HashMap};
	use std::io::{Error, ErrorKind};
	use std::sync::{Mutex, MutexGuard};

	use uuid::Uuid;

	use super::JobDb;

	type LocalMap<JOB, TASK> = HashMap<Uuid, (JOB, Vec<(TASK, Option<Uuid>, BTreeSet<u32>)>)>;

	pub struct LocalJobDb<JOB: Sync + Send + Clone, TASK: Sync + Send + Clone> {
		jobs: Mutex<LocalMap<JOB, TASK>>,
	}

	impl<JOB: Sync + Send + Clone, TASK: Sync + Send + Clone> Default for LocalJobDb<JOB, TASK> {
		fn default() -> Self {
			Self {
				jobs: Mutex::new(Default::default()),
			}
		}
	}

	impl<JOB: Sync + Send + Clone, TASK: Sync + Send + Clone> LocalJobDb<JOB, TASK> {
		fn lock(&self) -> MutexGuard<'_, LocalMap<JOB, TASK>> {
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

		async fn append_task(&self, job_id: &Uuid, task: TASK, dep: &[u32]) -> Result<u32, Error> {
			let mut guard = self.lock();
			let job = match guard.get_mut(job_id).map(|(_, tasks)| tasks) {
				None => return Err(Error::new(ErrorKind::NotFound, "Job not found")),
				Some(tasks) => tasks,
			};
			let idx = job.len();
			if dep.iter().any(|x| x >= &(idx as u32)) {
				return Err(Error::new(ErrorKind::NotFound, "Dependency not found"));
			}
			job.push((task, None, BTreeSet::from_iter(dep.iter().cloned())));
			Ok(idx as u32)
		}

		async fn get_tasks(&self, job_id: &Uuid) -> Result<Vec<TASK>, Error> {
			self.lock()
				.get(job_id)
				.map(|(_, tasks)| tasks.iter().map(|(task, _, _)| task).cloned().collect())
				.ok_or_else(|| Error::new(ErrorKind::NotFound, "Job not found"))
		}

		async fn get_allocated_task(
			&self,
			job_id: &Uuid,
			task_id: &Uuid,
		) -> Result<Option<(TASK, u32)>, Error> {
			let guard = self.lock();
			let job = guard
				.get(job_id)
				.ok_or_else(|| Error::new(ErrorKind::NotFound, "Job not found"))?;
			let task = job
				.1
				.iter()
				.enumerate()
				.find(|(i, (_task, id, _))| id.as_ref() == Some(task_id))
				.map(|(i, (task, _, _))| (task.clone(), i as u32));
			Ok(task)
		}

		async fn allocate_task(&self) -> Result<Option<(Uuid, Uuid)>, Error> {
			let mut binding = self.lock();
			let available = binding
				.iter_mut()
				.flat_map(|(job_id, (_, tasks))| {
					tasks
						.iter_mut()
						.filter(|(_, allocation, dependencies)| {
							allocation.is_none() && dependencies.is_empty()
						})
						.map(|task| (*job_id, task))
				})
				.next();
			match available {
				None => Ok(None),
				Some((job_id, available)) => {
					let id = Uuid::new_v4();
					available.1 = Some(id);
					Ok(Some((job_id, id)))
				}
			}
		}

		async fn fulfill(&self, job_id: &Uuid, task_idx: u32) -> Result<(), Error> {
			let mut binding = self.lock();
			let job = binding
				.get_mut(job_id)
				.map(|job| {
					let found_task = job.1.len() > task_idx as usize;
					found_task.then_some(job)
				})
				.unwrap_or_default()
				.ok_or_else(|| Error::new(ErrorKind::NotFound, "Task_not_found"))?;
			for (_, _, deps) in job.1.iter_mut().skip(task_idx as usize) {
				deps.remove(&task_idx);
			}
			Ok(())
		}

		async fn get_task_status(&self, job_id: &Uuid, task_idx: u32) -> Result<Option<()>, Error> {
			let binding = self.lock();
			binding
				.get(job_id)
				.map(|(_, tasks)| tasks.get(task_idx as usize))
				.unwrap_or_default()
				.ok_or_else(|| Error::new(ErrorKind::NotFound, "Job not found"))
				.and(Ok(None))
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
			let first_task = manager
				.append_task(&Uuid::from_u64_pair(1, 2), task, &[])
				.await;
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
			let task_idx = manager
				.append_task(&job_id, task.clone(), &[])
				.await
				.unwrap();
			let res = manager.get_task(&job_id, task_idx).await.unwrap();
			assert_eq!(task, res)
		}

		#[tokio::test]
		async fn add_task_with_dependency_that_does_not_exist_fails() {
			let manager = LocalJobDb::<String, String>::default();
			let task = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job.clone()).await.unwrap();
			let task_idx = manager.append_task(&job_id, task, &[1000]).await;
			assert!(task_idx.is_err());
		}

		#[tokio::test]
		async fn add_task_with_previous_as_dependency() {
			let manager = LocalJobDb::<String, String>::default();
			let task = "Task 1".to_string();
			let task2 = "Task 2".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job.clone()).await.unwrap();
			let task_idx = manager.append_task(&job_id, task, &[]).await.unwrap();
			let task2_idx = manager.append_task(&job_id, task2, &[task_idx]).await;
			assert!(task2_idx.is_ok());
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
			manager
				.append_task(&job_id, task_1.clone(), &[])
				.await
				.unwrap();
			manager
				.append_task(&job_id, task_2.clone(), &[])
				.await
				.unwrap();
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
			let first_task = manager.append_task(&id, task_1, &[]).await.unwrap();
			let second_task = manager.append_task(&id, task_2, &[]).await.unwrap();
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
		async fn allocate_task_returns_job_id_and_run_id() {
			let manager = LocalJobDb::<String, String>::default();
			let task = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let _task_idx = manager.append_task(&job_id, task, &[]).await.unwrap();
			let (allocated_job_id, allocation_id): (Uuid, Uuid) =
				manager.allocate_task().await.unwrap().unwrap();
			assert_eq!(allocated_job_id, job_id);
			assert!(!allocation_id.is_nil())
		}

		#[tokio::test]
		async fn allocate_more_than_available_return_none() {
			let manager = LocalJobDb::<String, String>::default();
			let task = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			manager.append_task(&job_id, task, &[]).await.unwrap();
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
			manager.append_task(&job_id, task_1, &[]).await.unwrap();
			manager.append_task(&job_id, task_2, &[]).await.unwrap();
			let allocated_1 = manager.allocate_task().await.unwrap();
			let allocated_2 = manager.allocate_task().await.unwrap();
			assert!(allocated_1.is_some());
			assert!(allocated_2.is_some());
		}

		#[tokio::test]
		async fn allocate_tasks_before_dependency_fulfill_returns_none() {
			let manager = LocalJobDb::<String, String>::default();
			let task_1 = "Task 1".to_string();
			let task_2 = "Task 2".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let idx = manager.append_task(&job_id, task_1, &[]).await.unwrap();
			manager.append_task(&job_id, task_2, &[idx]).await.unwrap();
			manager
				.allocate_task()
				.await
				.unwrap()
				.expect("Should allocate first");
			let allocated_2 = manager.allocate_task().await.unwrap();
			assert!(allocated_2.is_none());
		}

		#[tokio::test]
		async fn allocate_tasks_after_dependency_fulfill() {
			let manager = LocalJobDb::<String, String>::default();
			let task_1 = "Task 1".to_string();
			let task_2 = "Task 2".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let idx = manager.append_task(&job_id, task_1, &[]).await.unwrap();
			manager.append_task(&job_id, task_2, &[idx]).await.unwrap();
			manager
				.allocate_task()
				.await
				.unwrap()
				.expect("Should allocate first");
			manager.fulfill(&job_id, idx).await.unwrap();
			let allocated_2 = manager.allocate_task().await.unwrap();
			assert!(allocated_2.is_some());
		}

		#[tokio::test]
		async fn fulfill_invalid_task_error() {
			let manager = LocalJobDb::<String, String>::default();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let res = manager.fulfill(&job_id, 0).await;
			assert!(res.is_err());
		}

		#[tokio::test]
		async fn fulfill_success() {
			let manager = LocalJobDb::<String, String>::default();
			let task_1 = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let idx = manager.append_task(&job_id, task_1, &[]).await.unwrap();
			// let task_id = manager.allocate_task().await.unwrap().unwrap();
			let res = manager.fulfill(&job_id, idx).await;
			assert!(res.is_ok());
		}

		#[tokio::test]
		async fn can_allocate_tasks_after_dependency_fulfill() {
			let manager = LocalJobDb::<String, String>::default();
			let task_1 = "Task 1".to_string();
			let task_2 = "Task 2".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let idx = manager.append_task(&job_id, task_1, &[]).await.unwrap();
			let _idx2 = manager.append_task(&job_id, task_2, &[idx]).await.unwrap();
			manager
				.allocate_task()
				.await
				.unwrap()
				.expect("Should allocate first");
			manager.fulfill(&job_id, idx).await.unwrap();
			let allocated_2 = manager.allocate_task().await.unwrap();
			assert!(allocated_2.is_some());
		}

		#[tokio::test]
		async fn get_allocated_task_with_bad_job_fails() {
			let manager = LocalJobDb::<String, String>::default();
			let task: Result<_, _> = manager.get_allocated_task(&Uuid::nil(), &Uuid::nil()).await;
			assert!(task.is_err());
		}

		#[tokio::test]
		async fn get_allocated_task_with_bad_task_id_none() {
			let manager = LocalJobDb::<String, String>::default();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let task: Option<_> = manager
				.get_allocated_task(&job_id, &Uuid::nil())
				.await
				.unwrap();
			assert!(task.is_none());
		}

		#[tokio::test]
		async fn get_allocated_task_by_uuid() {
			let manager = LocalJobDb::<String, String>::default();
			let task_src = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let _task_idx = manager
				.append_task(&job_id, task_src.clone(), &[])
				.await
				.unwrap();
			let (job_id, task_id) = manager.allocate_task().await.unwrap().unwrap();
			let task = manager.get_allocated_task(&job_id, &task_id).await.unwrap();
			assert!(task.is_some());
			assert_eq!(task.unwrap().0, task_src);
		}

		#[tokio::test]
		async fn get_allocated_task_returns_task_idx() {
			let manager = LocalJobDb::<String, String>::default();
			let task_src = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let task_idx = manager
				.append_task(&job_id, task_src.clone(), &[])
				.await
				.unwrap();
			let (job_id, task_id) = manager.allocate_task().await.unwrap().unwrap();
			let (_task, idx) = manager
				.get_allocated_task(&job_id, &task_id)
				.await
				.unwrap()
				.unwrap();
			assert_eq!(idx, task_idx);
		}

		#[tokio::test]
		async fn get_task_status_before_set_returns_none() {
			let manager = LocalJobDb::<String, String>::default();
			let task_src = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let task_idx = manager
				.append_task(&job_id, task_src.clone(), &[])
				.await
				.unwrap();
			let status = manager.get_task_status(&job_id, task_idx).await.unwrap();
			assert!(status.is_none());
		}

		#[tokio::test]
		async fn get_task_status_bad_job_error() {
			let manager = LocalJobDb::<String, String>::default();
			let status = manager.get_task_status(&Uuid::nil(), 0).await;
			assert!(status.is_err());
		}

		#[tokio::test]
		async fn get_task_status_bad_task_error() {
			let manager = LocalJobDb::<String, String>::default();
			let task_src = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let task_idx = manager
				.append_task(&job_id, task_src.clone(), &[])
				.await
				.unwrap();
			let status = manager.get_task_status(&job_id, task_idx + 10).await;
			assert!(status.is_err());
		}
	}
}
