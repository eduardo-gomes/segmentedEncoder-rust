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

use std::future::Future;

use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct Allocated<JOB: Sync, TASK: Sync> {
	pub task: TASK,
	pub job: JOB,
	pub idx: u32,
}

#[cfg_attr(test, mockall::automock)]
pub trait JobDb<JOB: Sync, TASK: Sync, STATUS: Sync>: Sync {
	fn get_job(
		&self,
		id: &Uuid,
	) -> impl Future<Output = Result<Option<JOB>, std::io::Error>> + Send;
	fn create_job(&self, job: JOB) -> impl Future<Output = Result<Uuid, std::io::Error>> + Send;
	fn list_job_ids(&self) -> impl Future<Output = Result<Vec<Uuid>, std::io::Error>> + Send;
	/// Append task to job and return the task index
	fn append_task(
		&self,
		job_id: &Uuid,
		task: TASK,
		dep: &[u32],
	) -> impl Future<Output = Result<u32, std::io::Error>> + Send;
	fn get_tasks(
		&self,
		job_id: &Uuid,
	) -> impl Future<Output = Result<Option<Vec<TASK>>, std::io::Error>> + Send;
	fn get_task(
		&self,
		job_id: &Uuid,
		task_idx: u32,
	) -> impl Future<Output = Result<Option<TASK>, std::io::Error>> + Send {
		async move {
			let tasks = self.get_tasks(job_id).await?;
			Ok(tasks.and_then(|tasks| tasks.into_iter().nth(task_idx as usize)))
		}
	}
	fn get_allocated_task(
		&self,
		job_id: &Uuid,
		task_id: &Uuid,
	) -> impl Future<Output = Result<Option<Allocated<JOB, TASK>>, std::io::Error>> + Send;

	fn allocate_task(
		&self,
	) -> impl Future<Output = Result<Option<(Uuid, Uuid)>, std::io::Error>> + Send;
	///Mark the task as finished, allowing tasks that depend on this task to run
	fn fulfill(
		&self,
		job_id: &Uuid,
		task_idx: u32,
	) -> impl Future<Output = Result<(), std::io::Error>> + Send;
	fn get_task_status(
		&self,
		job_id: &Uuid,
		task_idx: u32,
	) -> impl Future<Output = Result<Option<STATUS>, std::io::Error>> + Send;
	fn set_task_status(
		&self,
		job_id: &Uuid,
		task_idx: u32,
		status: STATUS,
	) -> impl Future<Output = Result<Option<()>, std::io::Error>> + Send;
}

pub(crate) mod local {
	use std::collections::{BTreeSet, HashMap};
	use std::io::{Error, ErrorKind};
	use std::sync::{Mutex, MutexGuard};

	use uuid::Uuid;

	use super::{Allocated, JobDb};

	struct Entry<TASK, STATUS> {
		task: TASK,
		run_id: Option<Uuid>,
		dependencies: BTreeSet<u32>,
		status: Option<STATUS>,
	}

	type LocalMap<JOB, TASK, STATUS> = HashMap<Uuid, (JOB, Vec<Entry<TASK, STATUS>>)>;

	pub struct LocalJobDb<
		JOB: Sync + Send + Clone,
		TASK: Sync + Send + Clone,
		STATUS: Sync + Send + Clone,
	> {
		jobs: Mutex<LocalMap<JOB, TASK, STATUS>>,
	}

	impl<JOB: Sync + Send + Clone, TASK: Sync + Send + Clone, STATUS: Sync + Send + Clone> Default
		for LocalJobDb<JOB, TASK, STATUS>
	{
		fn default() -> Self {
			Self {
				jobs: Mutex::new(Default::default()),
			}
		}
	}

	impl<JOB: Sync + Send + Clone, TASK: Sync + Send + Clone, STATUS: Sync + Send + Clone>
		LocalJobDb<JOB, TASK, STATUS>
	{
		fn lock(&self) -> MutexGuard<'_, LocalMap<JOB, TASK, STATUS>> {
			self.jobs
				.lock()
				.unwrap_or_else(|poison| poison.into_inner())
		}
	}

	impl<JOB: Sync + Send + Clone, TASK: Sync + Send + Clone, STATUS: Sync + Send + Clone>
		JobDb<JOB, TASK, STATUS> for LocalJobDb<JOB, TASK, STATUS>
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

		async fn list_job_ids(&self) -> Result<Vec<Uuid>, Error> {
			Ok(self.lock().keys().cloned().collect())
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
			job.push(Entry {
				task,
				run_id: None,
				dependencies: BTreeSet::from_iter(dep.iter().cloned()),
				status: None,
			});
			Ok(idx as u32)
		}

		async fn get_tasks(&self, job_id: &Uuid) -> Result<Option<Vec<TASK>>, Error> {
			Ok(self
				.lock()
				.get(job_id)
				.map(|(_, tasks)| tasks.iter().map(|entry| &entry.task).cloned().collect()))
		}

		async fn get_allocated_task(
			&self,
			job_id: &Uuid,
			task_id: &Uuid,
		) -> Result<Option<Allocated<JOB, TASK>>, Error> {
			let guard = self.lock();
			let job = match guard.get(job_id) {
				None => {
					return Ok(None);
				}
				Some(job) => job,
			};
			let task = job
				.1
				.iter()
				.enumerate()
				.find(|(_, entry)| entry.run_id.as_ref() == Some(task_id))
				.map(|(i, entry)| Allocated {
					task: entry.task.clone(),
					job: job.0.clone(),
					idx: i as u32,
				});
			Ok(task)
		}

		async fn allocate_task(&self) -> Result<Option<(Uuid, Uuid)>, Error> {
			let mut binding = self.lock();
			let available = binding
				.iter_mut()
				.flat_map(|(job_id, (_, tasks))| {
					tasks
						.iter_mut()
						.filter(|entry| entry.run_id.is_none() && entry.dependencies.is_empty())
						.map(|task| (*job_id, task))
				})
				.next();
			match available {
				None => Ok(None),
				Some((job_id, available)) => {
					let id = Uuid::new_v4();
					available.run_id = Some(id);
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
			for entry in job.1.iter_mut().skip(task_idx as usize) {
				entry.dependencies.remove(&task_idx);
			}
			Ok(())
		}

		async fn get_task_status(
			&self,
			job_id: &Uuid,
			task_idx: u32,
		) -> Result<Option<STATUS>, Error> {
			let binding = self.lock();
			let task = binding
				.get(job_id)
				.map(|(_, tasks)| tasks.get(task_idx as usize))
				.unwrap_or_default()
				.map(|entry| entry.status.clone());
			task.ok_or_else(|| Error::new(ErrorKind::NotFound, "Task not found"))
		}

		async fn set_task_status(
			&self,
			job_id: &Uuid,
			task_idx: u32,
			status: STATUS,
		) -> Result<Option<()>, Error> {
			let mut binding = self.lock();
			let task = binding
				.get_mut(job_id)
				.map(|(_, tasks)| tasks.get_mut(task_idx as usize))
				.unwrap_or_default();
			Ok(task.map(|entry| entry.status.insert(status)).and(Some(())))
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
			let manager = LocalJobDb::<(), (), ()>::default();
			let res = manager.get_job(&Uuid::from_u64_pair(1, 1)).await.unwrap();
			assert!(res.is_none())
		}

		#[tokio::test]
		async fn get_job_after_create() {
			let manager = LocalJobDb::<String, (), ()>::default();
			let job = "Job 1".to_string();
			let id = manager.create_job(job.clone()).await.unwrap();
			let res = manager.get_job(&id).await.unwrap().unwrap();
			assert_eq!(res, job)
		}

		#[tokio::test]
		async fn add_task_to_nonexistent_job_error() {
			let manager = LocalJobDb::<String, String, ()>::default();
			let task = "Task 1".to_string();
			let first_task = manager
				.append_task(&Uuid::from_u64_pair(1, 2), task, &[])
				.await;
			assert_eq!(first_task.unwrap_err().kind(), ErrorKind::NotFound)
		}

		#[tokio::test]
		async fn get_task_nonexistent_job_none() {
			let manager = LocalJobDb::<String, String, ()>::default();
			let res = manager.get_task(&Uuid::from_u64_pair(1, 2), 0).await;
			assert!(res.unwrap().is_none())
		}

		#[tokio::test]
		async fn add_get_task_by_id() {
			let manager = LocalJobDb::<String, String, ()>::default();
			let task = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job.clone()).await.unwrap();
			let task_idx = manager
				.append_task(&job_id, task.clone(), &[])
				.await
				.unwrap();
			let res = manager.get_task(&job_id, task_idx).await.unwrap().unwrap();
			assert_eq!(task, res)
		}

		#[tokio::test]
		async fn add_task_with_dependency_that_does_not_exist_fails() {
			let manager = LocalJobDb::<String, String, ()>::default();
			let task = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job.clone()).await.unwrap();
			let task_idx = manager.append_task(&job_id, task, &[1000]).await;
			assert!(task_idx.is_err());
		}

		#[tokio::test]
		async fn add_task_with_previous_as_dependency() {
			let manager = LocalJobDb::<String, String, ()>::default();
			let task = "Task 1".to_string();
			let task2 = "Task 2".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job.clone()).await.unwrap();
			let task_idx = manager.append_task(&job_id, task, &[]).await.unwrap();
			let task2_idx = manager.append_task(&job_id, task2, &[task_idx]).await;
			assert!(task2_idx.is_ok());
		}

		#[tokio::test]
		async fn add_get_all_tasks_nonexistent_job_none() {
			let manager = LocalJobDb::<String, String, ()>::default();
			let error = manager.get_tasks(&Uuid::from_u64_pair(1, 3)).await.unwrap();
			assert!(error.is_none())
		}

		#[tokio::test]
		async fn add_get_all_tasks_of_job() {
			let manager = LocalJobDb::<String, String, ()>::default();
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
			let tasks = manager.get_tasks(&job_id).await.unwrap().unwrap();
			assert_eq!(tasks, [task_1, task_2])
		}

		#[tokio::test]
		async fn add_task_to_job_returns_sequencial_id() {
			let manager = LocalJobDb::<String, String, ()>::default();
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
			let manager = LocalJobDb::<String, String, ()>::default();
			let allocation = manager.allocate_task().await.unwrap();
			assert!(allocation.is_none())
		}

		#[tokio::test]
		async fn allocate_task_returns_job_id_and_run_id() {
			let manager = LocalJobDb::<String, String, ()>::default();
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
			let manager = LocalJobDb::<String, String, ()>::default();
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
			let manager = LocalJobDb::<String, String, ()>::default();
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
			let manager = LocalJobDb::<String, String, ()>::default();
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
			let manager = LocalJobDb::<String, String, ()>::default();
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
			let manager = LocalJobDb::<String, String, ()>::default();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let res = manager.fulfill(&job_id, 0).await;
			assert!(res.is_err());
		}

		#[tokio::test]
		async fn fulfill_success() {
			let manager = LocalJobDb::<String, String, ()>::default();
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
			let manager = LocalJobDb::<String, String, ()>::default();
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
		async fn get_allocated_task_with_bad_job_returns_none() {
			let manager = LocalJobDb::<String, String, ()>::default();
			let task = manager.get_allocated_task(&Uuid::nil(), &Uuid::nil()).await;
			assert_eq!(task.unwrap(), None);
		}

		#[tokio::test]
		async fn get_allocated_task_with_bad_task_id_none() {
			let manager = LocalJobDb::<String, String, ()>::default();
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
			let manager = LocalJobDb::<String, String, ()>::default();
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
			assert_eq!(task.unwrap().task, task_src);
		}

		#[tokio::test]
		async fn get_allocated_task_returns_task_idx() {
			let manager = LocalJobDb::<String, String, ()>::default();
			let task_src = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let task_idx = manager
				.append_task(&job_id, task_src.clone(), &[])
				.await
				.unwrap();
			let (job_id, task_id) = manager.allocate_task().await.unwrap().unwrap();
			let allocated = manager
				.get_allocated_task(&job_id, &task_id)
				.await
				.unwrap()
				.unwrap();
			assert_eq!(allocated.idx, task_idx);
		}

		#[tokio::test]
		async fn get_task_status_before_set_returns_none() {
			let manager = LocalJobDb::<String, String, ()>::default();
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
			let manager = LocalJobDb::<String, String, ()>::default();
			let status = manager.get_task_status(&Uuid::nil(), 0).await;
			assert!(status.is_err());
		}

		#[tokio::test]
		async fn get_task_status_bad_task_error() {
			let manager = LocalJobDb::<String, String, ()>::default();
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

		#[tokio::test]
		async fn set_task_status_before_set_ok() {
			let manager = LocalJobDb::<String, String, ()>::default();
			let task_src = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let task_idx = manager
				.append_task(&job_id, task_src.clone(), &[])
				.await
				.unwrap();
			let status = manager.set_task_status(&job_id, task_idx, ()).await;
			assert!(status.is_ok());
		}

		#[tokio::test]
		async fn set_task_status_bad_job_none() {
			let manager = LocalJobDb::<String, String, ()>::default();
			let status = manager.set_task_status(&Uuid::nil(), 0, ()).await.unwrap();
			assert!(status.is_none());
		}

		#[tokio::test]
		async fn set_task_status_bad_task_none() {
			let manager = LocalJobDb::<String, String, ()>::default();
			let task_src = "Task 1".to_string();
			let job = "Job 1".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let task_idx = manager
				.append_task(&job_id, task_src.clone(), &[])
				.await
				.unwrap();
			let status = manager
				.set_task_status(&job_id, task_idx + 10, ())
				.await
				.unwrap();
			assert!(status.is_none());
		}

		#[tokio::test]
		async fn get_task_status_after_set_equals() {
			let manager = LocalJobDb::<String, String, String>::default();
			let task_src = "Task 1".to_string();
			let job = "Job 1".to_string();
			let status = "Task state".to_string();
			let job_id = manager.create_job(job).await.unwrap();
			let task_idx = manager
				.append_task(&job_id, task_src.clone(), &[])
				.await
				.unwrap();
			manager
				.set_task_status(&job_id, task_idx, status.clone())
				.await
				.unwrap();
			let got = manager
				.get_task_status(&job_id, task_idx)
				.await
				.unwrap()
				.unwrap();
			assert_eq!(got, status);
		}

		#[tokio::test]
		async fn list_jobs_on_empty_returns_empty_list() {
			let manager = LocalJobDb::<String, String, String>::default();
			let ids = manager.list_job_ids().await.unwrap();
			assert!(ids.is_empty())
		}

		#[tokio::test]
		async fn list_jobs_on_non_empty_is_not_empty() {
			let manager = LocalJobDb::<String, String, String>::default();
			manager.create_job("JOB".to_string()).await.unwrap();
			let ids = manager.list_job_ids().await.unwrap();
			assert!(!ids.is_empty())
		}

		#[tokio::test]
		async fn list_jobs_has_the_right_id() {
			let manager = LocalJobDb::<String, String, String>::default();
			let id = manager.create_job("JOB".to_string()).await.unwrap();
			let ids = manager.list_job_ids().await.unwrap();
			assert!(ids.contains(&id))
		}
	}
}
