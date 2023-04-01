//! Manages jobs and job's tasks. Allocate, restart, and provide access to tasks from each job

use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use hyper::Body;
use tokio::sync::RwLock;
use uuid::Uuid;

pub(crate) use scheduler::AllocatedTaskRef;
pub(crate) use scheduler::JobScheduler;

use crate::jobs::{Job, JobParams, Source};
use crate::storage::Storage;

mod scheduler;

pub(crate) type JobManagerLock = RwLock<JobManager>;

#[async_trait]
pub(crate) trait JobManagerUtils {
	async fn create_job(
		&self,
		body: Body,
		params: JobParams,
	) -> io::Result<(Uuid, Arc<JobScheduler>)>;
}

#[async_trait]
impl JobManagerUtils for JobManagerLock {
	//TODO: store the file before calling create_job
	async fn create_job(
		&self,
		body: Body,
		params: JobParams,
	) -> io::Result<(Uuid, Arc<JobScheduler>)> {
		let file_ref = Storage::body_to_file(self, body).await?;
		let job = Job::new(Source::File(file_ref), params);

		Ok(self.write().await.add_job(job))
	}
}

pub(crate) struct JobManager {
	map: HashMap<Uuid, Arc<JobScheduler>>,
	pub storage: Storage,
}

impl Debug for JobManager {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		//map to use uuid as hyphenated
		let map: Vec<(String, &Arc<JobScheduler>)> = self
			.map
			.iter()
			.map(|(u, scheduler)| (u.as_hyphenated().to_string(), scheduler))
			.collect();
		f.debug_struct("JobManager")
			.field("count", &self.job_count())
			.field("map", &map)
			.finish()
	}
}

pub struct TaskId {
	pub job: Uuid,
	pub task: Uuid,
}

impl JobManager {
	pub(crate) fn status(&self) -> String {
		format!("{:#?}", self)
	}

	pub(crate) fn get_job(&self, uuid: &Uuid) -> Option<Arc<Job>> {
		self.map
			.get(uuid)
			.map(|scheduler| scheduler.get_job())
			.cloned()
	}
	pub(crate) async fn get_job_task(&self, job: &Uuid, task: &Uuid) -> Option<AllocatedTaskRef> {
		self.map.get(job)?.get_allocated(task).await
	}

	///Retuns the new job id, and the job scheduler for this job
	pub(crate) fn add_job(&mut self, job: Job) -> (Uuid, Arc<JobScheduler>) {
		let uuid = Uuid::new_v4();
		let arc = Arc::new(job);
		let job_scheduler = JobScheduler::new(arc.clone());
		let arc1 = Arc::new(job_scheduler);
		self.map.insert(uuid, arc1.clone());
		(uuid, arc1)
	}

	pub fn job_count(&self) -> usize {
		self.map.len()
	}

	pub(crate) async fn allocate(&self) -> Option<(TaskId, AllocatedTaskRef)> {
		{
			for (job_id, scheduler) in self.map.iter() {
				let allocated = scheduler.allocate().await;
				if allocated.is_some() {
					return allocated.map(|(task, allocated)| {
						(
							TaskId {
								job: job_id.clone(),
								task,
							},
							allocated,
						)
					});
				}
			}
		};
		None
	}

	pub fn new(storage: Storage) -> Self {
		JobManager {
			map: Default::default(),
			storage,
		}
	}

	pub(crate) fn get_task_scheduler(&self, job_id: &Uuid) -> Option<&Arc<JobScheduler>> {
		self.map.get(job_id).map(|scheduler| scheduler)
	}
}

#[cfg(test)]
mod test {
	use std::ops::Deref;
	use std::ptr;

	use hyper::Body;
	use tokio::io::AsyncReadExt;
	use tokio::sync::RwLock;
	use uuid::Uuid;

	use crate::jobs::manager::{AllocatedTaskRef, JobManager, JobManagerUtils};
	use crate::jobs::{Job, JobParams, Source};
	use crate::storage::FileRef;
	use crate::{Storage, WEBM_SAMPLE};

	fn make_job_manager() -> JobManager {
		JobManager::new(Storage::new().unwrap())
	}

	#[test]
	fn new_job_manager_has_0_jobs() {
		let manager = make_job_manager();

		assert_eq!(manager.job_count(), 0);
	}

	#[test]
	fn get_job_nonexistent_uuid_none() {
		let manager = make_job_manager();
		let uuid = Uuid::new_v4();
		let job = manager.get_job(&uuid);
		assert!(job.is_none());
	}

	#[tokio::test]
	async fn get_reference_to_job_from_uuid() {
		let mut manager = make_job_manager();
		let job = Job::new(Source::File(FileRef::fake()), JobParams::sample_params());

		let (uuid, job) = manager.add_job(job);
		let job2 = manager.get_job(&uuid).unwrap();
		assert!(
			ptr::eq(job.get_job().deref(), job2.deref()),
			"Should be reference to same object"
		);
	}

	#[tokio::test]
	async fn get_job_task_nonexistent_uuid_none() {
		let manager = make_job_manager();
		let uuid = Uuid::new_v4();
		let job = manager.get_job_task(&uuid, &uuid).await;
		assert!(job.is_none());
	}

	#[tokio::test]
	async fn get_reference_to_task_from_uuid() {
		let mut manager = make_job_manager();
		let job = Job::new(Source::File(FileRef::fake()), JobParams::sample_params());

		manager.add_job(job);
		let (id, allocated) = manager.allocate().await.expect("Should allocate");
		let task = manager.get_job_task(&id.job, &id.task).await.unwrap();
		assert!(
			ptr::eq(task.deref(), allocated.deref()),
			"Should be reference to same object"
		);
	}

	#[test]
	fn new_manager_has_1_job_after_enqueue() {
		let mut manager = make_job_manager();

		let job = Job::new(Source::File(FileRef::fake()), JobParams::sample_params());
		manager.add_job(job);
		assert_eq!(manager.job_count(), 1);
	}

	#[test]
	fn status_turns_into_string_with_job_id() {
		let mut manager = make_job_manager();
		let job = Job::new(Source::File(FileRef::fake()), JobParams::sample_params());
		let (uuid, _) = manager.add_job(job);

		let status = manager.status();
		let uuid_string = uuid.as_hyphenated().to_string();
		assert!(
			status.contains(&uuid_string),
			"'{status}' should contains '{uuid_string}'"
		)
	}

	#[tokio::test]
	async fn create_job() {
		let body = Body::from(WEBM_SAMPLE.as_slice());
		//Using rwlock because it will only lock for part of the function
		let manager = RwLock::new(make_job_manager());

		let (uuid, job) = manager
			.create_job(body, JobParams::sample_params())
			.await
			.unwrap();
		let job2 = manager.read().await.get_job(&uuid).unwrap();
		assert_eq!(job.get_job().deref(), job2.deref());
	}

	#[tokio::test]
	async fn create_job_check_source() {
		let body = Body::from(WEBM_SAMPLE.as_slice());
		//Using rwlock because it will only lock for part of the function
		let manager = RwLock::new(make_job_manager());

		let (_uuid, job) = manager
			.create_job(body, JobParams::sample_params())
			.await
			.unwrap();
		let job = job.get_job();
		let Source::File(uuid) = job.source.clone();
		let mut file = manager.read().await.storage.get_file(&uuid).await.unwrap();

		let mut content = Vec::new();
		file.read_to_end(&mut content).await.unwrap();

		assert_eq!(content, WEBM_SAMPLE);
	}

	#[tokio::test]
	async fn allocate_task_without_job_returns_none() {
		let manager = make_job_manager();
		let task = manager.allocate().await;
		assert!(task.is_none());
	}

	#[tokio::test]
	async fn allocate_task_with_do_not_segment_job_returns_task() {
		let mut manager = make_job_manager();
		let job = Job::new(Source::File(FileRef::fake()), JobParams::sample_params());
		manager.add_job(job);

		let task = manager.allocate().await;
		assert!(task.is_some());
	}

	#[tokio::test]
	async fn allocated_task_has_type_allocated_task() {
		let mut manager = make_job_manager();
		let job = Job::new(Source::File(FileRef::fake()), JobParams::sample_params());
		manager.add_job(job);

		let (_, task): (_, AllocatedTaskRef) = manager.allocate().await.unwrap();
		dbg!(task);
	}

	#[tokio::test]
	async fn allocated_task_has_job_and_task_ids() {
		let mut manager = make_job_manager();
		let job = Job::new(Source::File(FileRef::fake()), JobParams::sample_params());
		manager.add_job(job);

		let (id, task): (_, AllocatedTaskRef) = manager.allocate().await.unwrap();

		let got_from_id = manager
			.get_task_scheduler(&id.job)
			.expect("Should get job")
			.get_allocated(&id.task)
			.await
			.expect("Should get allocated task");
		assert!(
			ptr::eq(got_from_id.deref(), task.deref()),
			"Both should be the same object"
		);
	}

	#[tokio::test]
	async fn allocate_task_twice_with_one_do_not_segment_job_returns_one_time() {
		let mut manager = make_job_manager();
		let job = Job::new(Source::File(FileRef::fake()), JobParams::sample_params());
		manager.add_job(job);

		let _task = manager.allocate().await;
		let task = manager.allocate().await;
		assert!(task.is_none());
	}

	#[tokio::test]
	async fn allocate_task_twice_with_two_do_not_segment_job_returns_twice() {
		let mut manager = make_job_manager();
		manager.add_job(Job::new(
			Source::File(FileRef::fake()),
			JobParams::sample_params(),
		));
		manager.add_job(Job::new(
			Source::File(FileRef::fake()),
			JobParams::sample_params(),
		));

		let _task = manager.allocate().await;
		let task = manager.allocate().await;
		assert!(task.is_some());
	}

	#[tokio::test]
	async fn get_task_scheduler_get_task_give_the_same_task() {
		let mut manager = make_job_manager();
		let job = Job::new(Source::File(FileRef::fake()), JobParams::sample_params());
		let (job_id, _) = manager.add_job(job);
		let (id, task): (_, AllocatedTaskRef) = manager.allocate().await.unwrap();
		let scheduler = manager.get_task_scheduler(&job_id).unwrap();
		let got_task = scheduler
			.get_allocated(&id.task)
			.await
			.expect("Should have task");
		assert!(ptr::eq(got_task.deref(), task.deref()));
	}
}
