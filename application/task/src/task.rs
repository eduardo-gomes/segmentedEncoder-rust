//! #Task crate
//! This crate defines the tasks, and includes the task runner under a feature, and the job/task manager trait

use uuid::Uuid;

struct JobSource {}

enum TaskSource {
	Analysis(),
	Transcode(),
}

///An allocated task
struct TaskInstance {
	job_id: Uuid,
	task_id: Uuid,
}

trait Manager {
	fn crate_job(job: JobSource) -> Uuid;
	fn allocate_task() -> TaskInstance;
	fn add_task_to_job(job_id: &Uuid, task: TaskSource) -> u32;
	fn get_task(job_id: &Uuid, task_id: &Uuid) -> TaskInstance;
	fn update_task_status(job_id: &Uuid, task_id: &Uuid);
	fn set_task_output(job_id: &Uuid, task_id: &Uuid);
	///Cancel this task execution, will be available for allocation
	fn cancel_task(job_id: &Uuid, task_id: &Uuid);
	///Delete the job removing all tasks, completed or pending
	fn delete_job(job_id: &Uuid);
}
