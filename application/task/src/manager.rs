use uuid::Uuid;

use crate::{Instance, JobSource, TaskSource};

mod db;

///Interface used by the server to manage jobs and tasks
trait Manager {
	async fn crate_job(job: JobSource) -> Result<Uuid, std::io::Error>;
	async fn allocate_task() -> Result<Option<Instance>, std::io::Error>;
	async fn add_task_to_job(job_id: &Uuid, task: TaskSource) -> Result<u32, std::io::Error>;
	async fn get_task(job_id: &Uuid, task_id: &Uuid) -> Result<Instance, std::io::Error>;
	async fn update_task_status(job_id: &Uuid, task_id: &Uuid) -> Result<(), std::io::Error>;
	async fn set_task_output(job_id: &Uuid, task_id: &Uuid) -> Result<(), std::io::Error>;
	///Cancel this task execution, will be available for allocation
	async fn cancel_task(job_id: &Uuid, task_id: &Uuid) -> Result<(), std::io::Error>;
	///Delete the job removing all tasks, completed or pending
	async fn delete_job(job_id: &Uuid) -> Result<(), std::io::Error>;
}
