use std::io::Error;

use uuid::Uuid;

use crate::{Instance, JobSource, TaskSource};

mod db;

///Interface used by the server to manage jobs and tasks
trait Manager {
	async fn create_job(&self, job: JobSource) -> Result<Uuid, std::io::Error>;
	async fn allocate_task(&self) -> Result<Option<Instance>, std::io::Error>;
	async fn add_task_to_job(&self, job_id: &Uuid, task: TaskSource)
		-> Result<u32, std::io::Error>;
	async fn get_task(&self, job_id: &Uuid, task_id: &Uuid) -> Result<Instance, std::io::Error>;
	async fn update_task_status(&self, job_id: &Uuid, task_id: &Uuid)
		-> Result<(), std::io::Error>;
	async fn set_task_output(&self, job_id: &Uuid, task_id: &Uuid) -> Result<(), std::io::Error>;
	///Cancel this task execution, will be available for allocation
	async fn cancel_task(&self, job_id: &Uuid, task_id: &Uuid) -> Result<(), std::io::Error>;
	///Delete the job removing all tasks, completed or pending
	async fn delete_job(&self, job_id: &Uuid) -> Result<(), std::io::Error>;
}

struct JobManager<DB: db::JobDb<JobSource, TaskSource>> {
	db: DB,
}

impl<DB: db::JobDb<JobSource, TaskSource>> Manager for JobManager<DB> {
	async fn create_job(&self, job: JobSource) -> Result<Uuid, Error> {
		self.db.create_job(job).await
	}

	async fn allocate_task(&self) -> Result<Option<Instance>, Error> {
		match self.db.allocate_task().await? {
			Some((job_id, task_id)) => match self.db.get_allocated_task(&job_id, &task_id).await? {
				None => Ok(None),
				Some(task) => Ok(Some(Instance {
					job_id,
					task_id,
					inputs: task.0.inputs,
					recipe: task.0.recipe,
				})),
			},
			None => Ok(None),
		}
	}

	async fn add_task_to_job(&self, job_id: &Uuid, task: TaskSource) -> Result<u32, Error> {
		todo!()
	}

	async fn get_task(&self, job_id: &Uuid, task_id: &Uuid) -> Result<Instance, Error> {
		todo!()
	}

	async fn update_task_status(&self, job_id: &Uuid, task_id: &Uuid) -> Result<(), Error> {
		todo!()
	}

	async fn set_task_output(&self, job_id: &Uuid, task_id: &Uuid) -> Result<(), Error> {
		todo!()
	}

	async fn cancel_task(&self, job_id: &Uuid, task_id: &Uuid) -> Result<(), Error> {
		todo!()
	}

	async fn delete_job(&self, job_id: &Uuid) -> Result<(), Error> {
		todo!()
	}
}

#[cfg(test)]
mod test {
	use uuid::Uuid;

	use crate::manager::db::MockJobDb;
	use crate::manager::{JobManager, Manager};
	use crate::Recipe::Analysis;
	use crate::{Input, Instance, JobSource, Options, TaskSource};

	#[tokio::test]
	async fn create_job_uses_db_and_returns_uuid() {
		let source = JobSource {
			input_id: Uuid::from_u64_pair(1, 1),
			video_options: Options {
				codec: "libx264".to_string(),
				params: vec![],
			},
		};
		let mut mock = MockJobDb::new();
		const TARGET_ID: Uuid = Uuid::from_u64_pair(123, 123);
		mock.expect_create_job()
			.with(mockall::predicate::eq(source.clone()))
			.times(1)
			.returning(|_| Ok(TARGET_ID));
		let manager = JobManager { db: mock };
		let id = manager.create_job(source).await.unwrap();
		assert_eq!(id, TARGET_ID);
	}

	#[tokio::test]
	async fn allocate_task_no_available() {
		let mut mock = MockJobDb::new();
		mock.expect_allocate_task().times(1).returning(|| Ok(None));
		let manager = JobManager { db: mock };
		let instance = manager.allocate_task().await.unwrap();
		assert!(instance.is_none());
	}

	#[tokio::test]
	async fn allocate_task_returns_instance() {
		const JOB_ID: Uuid = Uuid::from_u64_pair(1, 1);
		const TASK_ID: Uuid = Uuid::from_u64_pair(1, 2);
		const INPUT: Input = Input {
			index: 0,
			start: None,
			end: None,
		};
		let task: TaskSource = TaskSource {
			inputs: vec![INPUT],
			recipe: Analysis(),
		};
		let target_instance = Instance {
			job_id: JOB_ID,
			task_id: TASK_ID,
			inputs: task.inputs.clone(),
			recipe: task.recipe.clone(),
		};
		let mut mock = MockJobDb::new();

		mock.expect_allocate_task()
			.times(1)
			.returning(|| Ok(Some((JOB_ID, TASK_ID))));
		mock.expect_get_allocated_task()
			.withf(|a, b| *a == JOB_ID && *b == TASK_ID)
			.times(1)
			.returning(|_job_id, _task_id| {
				Ok(Some((
					TaskSource {
						inputs: vec![INPUT],
						recipe: Analysis(),
					},
					0,
				)))
			});
		let manager = JobManager { db: mock };
		let instance = manager.allocate_task().await.unwrap().unwrap();
		assert_eq!(instance, target_instance);
	}
}
