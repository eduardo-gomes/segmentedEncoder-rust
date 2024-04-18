use std::io::{Error, ErrorKind};

use uuid::Uuid;

use crate::manager::db::local::LocalJobDb;
use crate::{Instance, JobSource, Status, TaskSource};

mod db;

///Interface used by the server to manage jobs and tasks
pub trait Manager: Sync {
	fn create_job(
		&self,
		job: JobSource,
	) -> impl std::future::Future<Output = Result<Uuid, Error>> + Send;
	fn get_job(
		&self,
		job_id: &Uuid,
	) -> impl std::future::Future<Output = Result<Option<JobSource>, Error>> + Send;
	fn allocate_task(
		&self,
	) -> impl std::future::Future<Output = Result<Option<Instance>, Error>> + Send;
	fn add_task_to_job(
		&self,
		job_id: &Uuid,
		task: TaskSource,
	) -> impl std::future::Future<Output = Result<u32, Error>> + Send;
	fn get_task_source(
		&self,
		job_id: &Uuid,
		task: u32,
	) -> impl std::future::Future<Output = Result<Option<TaskSource>, Error>> + Send;
	fn get_task(
		&self,
		job_id: &Uuid,
		task_id: &Uuid,
	) -> impl std::future::Future<Output = Result<Option<Instance>, Error>> + Send;
	fn update_task_status(
		&self,
		job_id: &Uuid,
		task_id: &Uuid,
		status: Status,
	) -> impl std::future::Future<Output = Result<(), Error>> + Send;
	fn set_task_output(
		&self,
		job_id: &Uuid,
		task_id: &Uuid,
		output: Uuid,
	) -> impl std::future::Future<Output = Result<Option<()>, Error>> + Send;
	fn get_task_output(
		&self,
		job_id: &Uuid,
		task_idx: u32,
	) -> impl std::future::Future<Output = Result<Option<Uuid>, Error>> + Send;
	fn get_task_input(
		&self,
		job_id: &Uuid,
		task_idx: u32,
		input_idx: u32,
	) -> impl std::future::Future<Output = Result<Uuid, Error>> + Send {
		async move {
			let err = || Error::new(ErrorKind::NotFound, "Input out of bounds");
			let task = self
				.get_task_source(&job_id, task_idx)
				.await?
				.ok_or_else(err)?;
			let _input = task.inputs.get(input_idx as usize).ok_or_else(err)?;
			let job_input = self
				.get_job(&job_id)
				.await?
				.ok_or(Error::new(
					ErrorKind::NotFound,
					"Job deleted during operation",
				))?
				.input_id;
			Ok(job_input)
		}
	}
	fn get_allocated_task_input(
		&self,
		job_id: &Uuid,
		task_id: &Uuid,
		input_idx: u32,
	) -> impl std::future::Future<Output = Result<Option<Uuid>, Error>> + Send;
	///Cancel this task execution, will be available for allocation
	fn cancel_task(
		&self,
		job_id: &Uuid,
		task_id: &Uuid,
	) -> impl std::future::Future<Output = Result<Option<()>, Error>> + Send;
	///Delete the job removing all tasks, completed or pending
	fn delete_job(
		&self,
		job_id: &Uuid,
	) -> impl std::future::Future<Output = Result<Option<()>, Error>> + Send;
}

#[derive(Clone)]
pub struct TaskState {
	output: Option<Uuid>,
}

pub type LocalJobManager = JobManager<LocalJobDb<JobSource, TaskSource, TaskState>>;

impl Default for LocalJobManager {
	fn default() -> Self {
		LocalJobManager {
			db: Default::default(),
		}
	}
}

pub struct JobManager<DB: db::JobDb<JobSource, TaskSource, TaskState>> {
	db: DB,
}

impl<DB: db::JobDb<JobSource, TaskSource, TaskState> + Sync> Manager for JobManager<DB> {
	async fn create_job(&self, job: JobSource) -> Result<Uuid, Error> {
		self.db.create_job(job).await
	}

	async fn get_job(&self, job_id: &Uuid) -> Result<Option<JobSource>, Error> {
		self.db.get_job(job_id).await
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
		let deps: Vec<_> = task
			.inputs
			.iter()
			.map(|input| input.index)
			.filter(|zero| *zero != 0)
			.collect();
		self.db.append_task(job_id, task, deps.as_slice()).await
	}

	async fn get_task_source(&self, job_id: &Uuid, task: u32) -> Result<Option<TaskSource>, Error> {
		self.db.get_task(job_id, task).await
	}

	async fn get_task(&self, job_id: &Uuid, task_id: &Uuid) -> Result<Option<Instance>, Error> {
		self.db
			.get_allocated_task(job_id, task_id)
			.await
			.map(|opt| {
				opt.map(|(task, _)| Instance {
					job_id: *job_id,
					task_id: *task_id,
					inputs: task.inputs,
					recipe: task.recipe,
				})
			})
	}

	async fn update_task_status(
		&self,
		job_id: &Uuid,
		task_id: &Uuid,
		status: Status,
	) -> Result<(), Error> {
		if let Status::Finished = status {
			match self
				.db
				.get_allocated_task(job_id, task_id)
				.await?
				.map(|(_, idx)| idx)
			{
				Some(idx) => self.db.fulfill(job_id, idx).await,
				None => Err(Error::new(ErrorKind::NotFound, "Task not found")),
			}
		} else {
			Err(Error::new(ErrorKind::NotFound, "Task not found"))
		}
	}

	async fn set_task_output(
		&self,
		job_id: &Uuid,
		task_id: &Uuid,
		output: Uuid,
	) -> Result<Option<()>, Error> {
		let idx = self
			.db
			.get_allocated_task(job_id, task_id)
			.await?
			.map(|(_, idx)| idx)
			.unwrap_or(u32::MAX /*NOT FOUND*/);
		self.db
			.set_task_status(
				job_id,
				idx,
				TaskState {
					output: Some(output),
				},
			)
			.await
	}

	async fn get_task_output(&self, job_id: &Uuid, task_idx: u32) -> Result<Option<Uuid>, Error> {
		Ok(self
			.db
			.get_task_status(job_id, task_idx)
			.await?
			.and_then(|status| status.output))
	}

	async fn get_allocated_task_input(
		&self,
		job_id: &Uuid,
		task_id: &Uuid,
		input_idx: u32,
	) -> Result<Option<Uuid>, Error> {
		let task = self.db.get_allocated_task(&job_id, &task_id).await?;
		Ok(match task {
			None => None,
			Some((_, task_idx)) => Some(self.get_task_input(&job_id, task_idx, input_idx).await?),
		})
	}

	async fn cancel_task(&self, job_id: &Uuid, task_id: &Uuid) -> Result<Option<()>, Error> {
		todo!()
	}

	async fn delete_job(&self, job_id: &Uuid) -> Result<Option<()>, Error> {
		todo!()
	}
}

#[cfg(test)]
mod test {
	use uuid::Uuid;

	use crate::manager::db::{JobDb, MockJobDb};
	use crate::manager::{JobManager, Manager};
	use crate::Recipe::{Analysis, Merge};
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
			.returning(|_| Box::pin(async { Ok(TARGET_ID) }));
		let manager = JobManager { db: mock };
		let id = manager.create_job(source).await.unwrap();
		assert_eq!(id, TARGET_ID);
	}

	#[tokio::test]
	async fn get_job_bad_id_returns_none() {
		let mut mock = MockJobDb::new();
		const TARGET_ID: Uuid = Uuid::from_u64_pair(123, 123);
		mock.expect_get_job()
			.with(mockall::predicate::eq(TARGET_ID))
			.times(1)
			.returning(|_| Box::pin(async { Ok(None) }));
		let manager = JobManager { db: mock };
		let job = manager.get_job(&TARGET_ID).await.unwrap();
		assert!(job.is_none());
	}

	#[tokio::test]
	async fn allocate_task_no_available() {
		let mut mock = MockJobDb::new();
		mock.expect_allocate_task()
			.times(1)
			.returning(|| Box::pin(async { Ok(None) }));
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
			recipe: Analysis(None),
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
			.returning(|| Box::pin(async { Ok(Some((JOB_ID, TASK_ID))) }));
		mock.expect_get_allocated_task()
			.withf(|a, b| *a == JOB_ID && *b == TASK_ID)
			.times(1)
			.returning(|_job_id, _task_id| {
				Box::pin(async {
					Ok(Some((
						TaskSource {
							inputs: vec![INPUT],
							recipe: Analysis(None),
						},
						0,
					)))
				})
			});
		let manager = JobManager { db: mock };
		let instance = manager.allocate_task().await.unwrap().unwrap();
		assert_eq!(instance, target_instance);
	}

	#[tokio::test]
	async fn add_task_to_job_passes_to_db() {
		const JOB_ID: Uuid = Uuid::from_u64_pair(1, 1);
		const IDX: u32 = 0;
		const INPUT: Input = Input {
			index: 0,
			start: None,
			end: None,
		};
		let task: TaskSource = TaskSource {
			inputs: vec![INPUT],
			recipe: Analysis(None),
		};
		let mut mock = MockJobDb::new();

		mock.expect_append_task()
			.withf(|job_id, task: &TaskSource, deps| {
				job_id == &JOB_ID && task.inputs[0] == INPUT && deps.is_empty()
			})
			.times(1)
			.returning(|_, _, _| Box::pin(async { Ok(IDX) }));
		let manager = JobManager { db: mock };
		let idx = manager.add_task_to_job(&JOB_ID, task).await.unwrap();
		assert_eq!(idx, IDX);
	}

	#[tokio::test]
	async fn add_task_specify_dependencies_based_on_inputs() {
		const JOB_ID: Uuid = Uuid::from_u64_pair(1, 1);
		const INPUT_1: Input = Input {
			index: 1,
			start: None,
			end: None,
		};
		const INPUT_2: Input = Input {
			index: 2,
			start: None,
			end: None,
		};
		let task: TaskSource = TaskSource {
			inputs: vec![INPUT_1, INPUT_2],
			recipe: Merge(vec![1, 2]),
		};
		let mut mock = MockJobDb::new();

		mock.expect_append_task()
			.withf(|_job_id, _task, deps| deps.contains(&1) && deps.contains(&2))
			.times(1)
			.returning(|_, _, _| Box::pin(async { Ok(3) }));
		let manager = JobManager { db: mock };
		manager.add_task_to_job(&JOB_ID, task).await.unwrap();
	}

	#[tokio::test]
	async fn add_task_input_0_has_no_dependencies() {
		const INPUT: Input = Input {
			index: 0,
			start: None,
			end: None,
		};
		let task: TaskSource = TaskSource {
			inputs: vec![INPUT],
			recipe: Analysis(None),
		};
		let mut mock = MockJobDb::new();

		mock.expect_append_task()
			.withf(|_job_id, _task, deps| deps.is_empty())
			.times(1)
			.returning(|_, _, _| Box::pin(async { Ok(0) }));
		let manager = JobManager { db: mock };
		manager
			.add_task_to_job(&Uuid::from_u64_pair(1, 1), task)
			.await
			.unwrap();
	}

	#[tokio::test]
	async fn get_task_returns_equals_the_allocated_task() {
		const INPUT: Input = Input {
			index: 0,
			start: None,
			end: None,
		};
		let task: TaskSource = TaskSource {
			inputs: vec![INPUT],
			recipe: Analysis(None),
		};

		let db = super::db::local::LocalJobDb::default();
		let job_id = db
			.create_job(JobSource {
				input_id: Uuid::from_u64_pair(1, 1),
				video_options: Options {
					codec: "libx264".to_string(),
					params: vec![],
				},
			})
			.await
			.unwrap();
		db.append_task(&job_id, task, &[]).await.unwrap();
		let manager = JobManager { db };
		let instance = manager.allocate_task().await.unwrap().unwrap();
		let got = manager
			.get_task(&job_id, &instance.task_id)
			.await
			.unwrap()
			.unwrap();
		assert_eq!(got, instance);
	}

	#[tokio::test]
	async fn get_task_unknown_task_returns_none() {
		let db = super::db::local::LocalJobDb::default();
		let job_id = db
			.create_job(JobSource {
				input_id: Uuid::from_u64_pair(1, 1),
				video_options: Options {
					codec: "libx264".to_string(),
					params: vec![],
				},
			})
			.await
			.unwrap();
		let manager = JobManager { db };
		let none = manager
			.get_task(&job_id, &Uuid::from_u64_pair(1, 2))
			.await
			.unwrap();
		assert!(none.is_none());
	}

	mod task_source {
		use crate::manager::LocalJobManager;
		use crate::Recipe;

		use super::*;

		#[tokio::test]
		async fn get_with_invalid_job_none() {
			let manager = LocalJobManager::default();
			let job_id = Uuid::nil();
			let res = manager.get_task_source(&job_id, 0).await.unwrap();
			assert!(res.is_none())
		}

		#[tokio::test]
		async fn job_without_task_none() {
			let manager = LocalJobManager::default();
			let job_id = manager
				.create_job(JobSource {
					input_id: Default::default(),
					video_options: Options {
						codec: "".to_string(),
						params: vec![],
					},
				})
				.await
				.unwrap();
			let res = manager.get_task_source(&job_id, 0).await.unwrap();
			assert!(res.is_none())
		}

		#[tokio::test]
		async fn get_with_valid_job_task_returns_the_task_source() {
			let manager = LocalJobManager::default();
			let job_id = manager
				.create_job(JobSource {
					input_id: Default::default(),
					video_options: Options {
						codec: "".to_string(),
						params: vec![],
					},
				})
				.await
				.unwrap();
			let task_source = TaskSource {
				inputs: vec![],
				recipe: Recipe::Analysis(None),
			};
			let task = manager
				.add_task_to_job(&job_id, task_source.clone())
				.await
				.unwrap();
			let res = manager
				.get_task_source(&job_id, task)
				.await
				.unwrap()
				.unwrap();
			assert_eq!(res, task_source)
		}
	}

	mod task_output {
		use crate::manager::db::local::LocalJobDb;

		use super::*;

		#[tokio::test]
		async fn get_task_output_bad_job_err() {
			let db = LocalJobDb::default();
			const JOB_ID: Uuid = Uuid::from_u64_pair(1, 1);
			let manager = JobManager { db };
			let res = manager.get_task_output(&JOB_ID, 0).await;
			assert!(res.is_err())
		}

		#[tokio::test]
		async fn get_task_output_bad_idx_err() {
			let db = LocalJobDb::default();
			let job_id = db
				.create_job(JobSource {
					input_id: Default::default(),
					video_options: Options {
						codec: "".to_string(),
						params: vec![],
					},
				})
				.await
				.unwrap();
			let manager = JobManager { db };
			let res = manager.get_task_output(&job_id, 0).await;
			assert!(res.is_err())
		}

		#[tokio::test]
		async fn get_task_output_before_set() {
			let db = LocalJobDb::default();
			let job_id = db
				.create_job(JobSource {
					input_id: Default::default(),
					video_options: Options {
						codec: "".to_string(),
						params: vec![],
					},
				})
				.await
				.unwrap();
			let idx = db
				.append_task(
					&job_id,
					TaskSource {
						inputs: vec![],
						recipe: Analysis(None),
					},
					&[],
				)
				.await
				.unwrap();
			let manager = JobManager { db };
			let output = manager.get_task_output(&job_id, idx).await.unwrap();
			assert!(output.is_none())
		}

		#[tokio::test]
		async fn get_task_output_after_set_equals() {
			let db = LocalJobDb::default();
			let job_id = db
				.create_job(JobSource {
					input_id: Default::default(),
					video_options: Options {
						codec: "".to_string(),
						params: vec![],
					},
				})
				.await
				.unwrap();
			let idx = db
				.append_task(
					&job_id,
					TaskSource {
						inputs: vec![],
						recipe: Analysis(None),
					},
					&[],
				)
				.await
				.unwrap();
			let (job_id, task_id) = db.allocate_task().await.unwrap().unwrap();
			let manager = JobManager { db };
			let output = Uuid::from_u64_pair(1, 3);
			manager
				.set_task_output(&job_id, &task_id, output)
				.await
				.unwrap();
			let got = manager
				.get_task_output(&job_id, idx)
				.await
				.unwrap()
				.expect("Should get the output");
			assert_eq!(got, output);
		}
	}

	mod task_input {
		use uuid::Uuid;

		use crate::manager::{LocalJobManager, Manager};
		use crate::{Input, JobSource, Options, Recipe, TaskSource};

		#[tokio::test]
		async fn with_invalid_job_err() {
			let manager = LocalJobManager::default();
			let job_id = Uuid::nil();
			let task = 0;
			let idx = 0;
			let input = manager.get_task_input(&job_id, task, idx).await;
			assert!(input.is_err())
		}

		#[tokio::test]
		async fn job_without_task_err() {
			let manager = LocalJobManager::default();
			let job_id = manager
				.create_job(JobSource {
					input_id: Default::default(),
					video_options: Options {
						codec: "".to_string(),
						params: vec![],
					},
				})
				.await
				.unwrap();
			let task = 0;
			let idx = 0;
			let input = manager.get_task_input(&job_id, task, idx).await;
			assert!(input.is_err())
		}

		#[tokio::test]
		async fn job_with_input_out_of_bounds_err() {
			let manager = LocalJobManager::default();
			let job_id = manager
				.create_job(JobSource {
					input_id: Default::default(),
					video_options: Options {
						codec: "".to_string(),
						params: vec![],
					},
				})
				.await
				.unwrap();
			let task = manager
				.add_task_to_job(
					&job_id,
					TaskSource {
						inputs: vec![Input::source()],
						recipe: Recipe::Analysis(None),
					},
				)
				.await
				.unwrap();
			let input = manager.get_task_input(&job_id, task, 1000).await;
			assert!(input.is_err());
		}

		#[tokio::test]
		async fn input_for_source_will_be_the_job_input() {
			let manager = LocalJobManager::default();
			let job_input = Uuid::from_u64_pair(123, 123);
			let job_id = manager
				.create_job(JobSource {
					input_id: job_input,
					video_options: Options {
						codec: "".to_string(),
						params: vec![],
					},
				})
				.await
				.unwrap();
			let task = manager
				.add_task_to_job(
					&job_id,
					TaskSource {
						inputs: vec![Input::source()],
						recipe: Recipe::Analysis(None),
					},
				)
				.await
				.unwrap();
			let input = manager.get_task_input(&job_id, task, 0).await.unwrap();
			assert_eq!(input, job_input)
		}

		#[tokio::test]
		async fn for_invalid_allocated_task_returns_none() {
			let manager = LocalJobManager::default();
			let job_id = manager
				.create_job(JobSource {
					input_id: Default::default(),
					video_options: Options {
						codec: "".to_string(),
						params: vec![],
					},
				})
				.await
				.unwrap();
			let task = 0;
			let idx = 0;
			let input = manager
				.get_allocated_task_input(&job_id, &Uuid::nil(), idx)
				.await
				.unwrap();
			assert!(input.is_none())
		}

		#[tokio::test]
		async fn return_same_content_for_allocated_task_by_its_idx() {
			let manager = LocalJobManager::default();
			let job_id = manager
				.create_job(JobSource {
					input_id: Uuid::from_u64_pair(1, 2),
					video_options: Options {
						codec: "".to_string(),
						params: vec![],
					},
				})
				.await
				.unwrap();
			let task = manager
				.add_task_to_job(
					&job_id,
					TaskSource {
						inputs: vec![Input::source()],
						recipe: Recipe::Analysis(None),
					},
				)
				.await
				.unwrap();
			let idx = 0;
			let task_id = manager.allocate_task().await.unwrap().unwrap().task_id;
			let input = manager
				.get_allocated_task_input(&job_id, &task_id, idx)
				.await
				.unwrap()
				.unwrap();
			let input_by_idx = manager.get_task_input(&job_id, task, idx).await.unwrap();
			assert_eq!(input, input_by_idx)
		}
	}
}
