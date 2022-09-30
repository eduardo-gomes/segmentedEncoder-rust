use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, RwLock};

use uuid::Uuid;

use crate::jobs::Job;

pub(crate) struct JobManager {
	count: usize,
	map: HashMap<Uuid, Arc<RwLock<Job>>>,
}

impl Debug for JobManager {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		//map to use uuid as hyphenated
		let map: Vec<(String, &Arc<RwLock<Job>>)> = self
			.map
			.iter()
			.map(|(u, j)| (u.as_hyphenated().to_string(), j))
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
}

impl JobManager {
	pub(crate) fn get_job(&self, uuid: &Uuid) -> Option<Arc<RwLock<Job>>> {
		self.map.get(uuid).map(|r| r.clone())
	}
}

impl JobManager {
	pub(crate) fn add_job(&mut self, job: Job) -> (Uuid, Arc<RwLock<Job>>) {
		self.count += 1;
		let uuid = Uuid::new_v4();
		let arc = Arc::new(RwLock::new(job));
		self.map.insert(uuid.clone(), arc.clone());
		(uuid, arc)
	}
}

impl JobManager {
	pub fn job_count(&self) -> usize {
		self.count
	}

	pub fn new() -> Self {
		JobManager {
			count: 0,
			map: Default::default(),
		}
	}
}

#[cfg(test)]
mod test {
	use std::ops::Deref;

	use uuid::Uuid;

	use crate::job_manager::JobManager;
	use crate::jobs::{Job, JobParams, Source};

	#[test]
	fn new_job_manager_has_0_jobs() {
		let manager = JobManager::new();

		assert_eq!(manager.job_count(), 0);
	}

	#[test]
	fn get_job_nonexistent_uuid_none() {
		let manager = JobManager::new();
		let uuid = Uuid::new_v4();
		let job = manager.get_job(&uuid);
		assert!(job.is_none());
	}

	#[test]
	fn get_reference_to_job_from_uuid() {
		let mut manager = JobManager::new();
		let job = Job::new(Source::Local(Uuid::nil()), JobParams::sample_params());

		let (uuid, job) = manager.add_job(job);
		let job2 = manager.get_job(&uuid).unwrap();
		assert!(
			std::ptr::eq(job.read().unwrap().deref(), job2.read().unwrap().deref()),
			"Should be reference to same object"
		);
	}

	#[test]
	fn new_manager_has_1_job_after_enqueue() {
		let mut manager = JobManager::new();

		let job = Job::new(Source::Local(Uuid::nil()), JobParams::sample_params());
		manager.add_job(job);
		assert_eq!(manager.job_count(), 1);
	}

	#[test]
	fn status_turns_into_string_with_job_id() {
		let mut manager = JobManager::new();
		let job = Job::new(Source::Local(Uuid::nil()), JobParams::sample_params());
		let (uuid, _) = manager.add_job(job);

		let status = manager.status().to_string();
		let uuid_string = uuid.as_hyphenated().to_string();
		assert!(
			status.contains(&uuid_string),
			"'{status}' should contains '{uuid_string}'"
		)
	}
}
