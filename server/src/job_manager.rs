use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::io;
use std::sync::Arc;

use async_trait::async_trait;
use hyper::Body;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::job_manager::old_task_scheduler::OldTaskScheduler;
use crate::jobs::{Job, JobParams, Source, Task};
use crate::storage::{stream, Storage};

mod job_scheduler;

pub(crate) type JobManagerLock = RwLock<JobManager>;

#[async_trait]
pub(crate) trait JobManagerUtils {
	async fn create_job(&self, body: Body, params: JobParams) -> io::Result<(Uuid, Arc<Job>)>;
}

#[async_trait]
impl JobManagerUtils for JobManagerLock {
	async fn create_job(&self, body: Body, params: JobParams) -> io::Result<(Uuid, Arc<Job>)> {
		let (mut file, id) = {
			let read = self.read().await;
			let res = read.storage.create_file();
			res.await?
		};
		stream::body_to_file(body, &mut file).await?;
		let job = Job::new(Source::File(id), params);

		Ok(self.write().await.add_job(job))
	}
}

pub(crate) struct JobManager {
	map: HashMap<Uuid, (Arc<Job>, Arc<OldTaskScheduler>)>,
	pub storage: Storage,
}

impl Debug for JobManager {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		//map to use uuid as hyphenated
		let map: Vec<(String, &Arc<Job>)> = self
			.map
			.iter()
			.map(|(u, (job, _))| (u.as_hyphenated().to_string(), job))
			.collect();
		f.debug_struct("JobManager")
			.field("count", &self.job_count())
			.field("map", &map)
			.finish()
	}
}

impl JobManager {
	pub(crate) fn status(&self) -> String {
		format!("{:#?}", self)
	}

	pub(crate) fn get_job(&self, uuid: &Uuid) -> Option<Arc<Job>> {
		self.map.get(uuid).map(|(job, _)| job).cloned()
	}

	pub(crate) fn add_job(&mut self, job: Job) -> (Uuid, Arc<Job>) {
		let uuid = Uuid::new_v4();
		let arc = Arc::new(job);
		let task_scheduler = arc.clone().make_segmenter_and_scheduler(uuid);
		self.map
			.insert(uuid, (arc.clone(), Arc::new(task_scheduler)));
		(uuid, arc)
	}

	pub fn job_count(&self) -> usize {
		self.map.len()
	}

	pub(crate) async fn allocate(&self) -> Option<Task> {
		{
			for (_, task_scheduler) in self.map.values() {
				let allocated = task_scheduler.allocate().await;
				if allocated.is_some() {
					return allocated;
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

	pub(crate) fn get_task_scheduler(&self, job_id: &Uuid) -> Option<&Arc<OldTaskScheduler>> {
		self.map.get(job_id).map(|(_, scheduler)| scheduler)
	}
}

#[cfg(test)]
mod test {
	use std::ops::Deref;

	use hyper::Body;
	use tokio::io::AsyncReadExt;
	use tokio::sync::RwLock;
	use uuid::Uuid;

	use crate::job_manager::{JobManager, JobManagerUtils};
	use crate::jobs::{Job, JobParams, Source, Task};
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
			std::ptr::eq(job.deref(), job2.deref()),
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
		assert_eq!(job.deref(), job2.deref());
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
	async fn allocated_task_has_type_task() {
		let mut manager = make_job_manager();
		let job = Job::new(Source::File(FileRef::fake()), JobParams::sample_params());
		manager.add_job(job);

		let task: Task = manager.allocate().await.unwrap();
		dbg!(task);
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
		manager.add_job(job);
		let task: Task = manager.allocate().await.unwrap();

		let scheduler = manager.get_task_scheduler(&task.job_id).unwrap();
		let got_task = scheduler
			.get_task(&task.id)
			.await
			.expect("Should have task");
		assert_eq!(got_task, task)
	}
}

#[deprecated]
mod old_task_scheduler;
