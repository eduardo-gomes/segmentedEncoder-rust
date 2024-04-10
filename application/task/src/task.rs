//! #Task crate
//! This crate defines the tasks, and includes the task runner under a feature, and the job/task manager trait

use uuid::Uuid;

struct JobSource {
	input_id: Uuid,
	video_options: Options,
}

struct TaskSource {
	///Here, the input should be the task id, or 0 for the job source
	inputs: Vec<Input>,
	recipe: Recipe,
}

struct Options {
	codec: String,
	params: Vec<String>,
}

enum Recipe {
	Analysis(),
	Transcode(Options),
	Merge(Vec<u32>),
}

struct Input {
	index: u32,
	start: Option<f64>,
	end: Option<f64>,
}

///An allocated task
struct Instance {
	job_id: Uuid,
	task_id: Uuid,
	inputs: Vec<Input>,
	recipe: Recipe,
}

trait Manager {
	fn crate_job(job: JobSource) -> Uuid;
	fn allocate_task() -> Instance;
	fn add_task_to_job(job_id: &Uuid, task: TaskSource) -> u32;
	fn get_task(job_id: &Uuid, task_id: &Uuid) -> Instance;
	fn update_task_status(job_id: &Uuid, task_id: &Uuid);
	fn set_task_output(job_id: &Uuid, task_id: &Uuid);
	///Cancel this task execution, will be available for allocation
	fn cancel_task(job_id: &Uuid, task_id: &Uuid);
	///Delete the job removing all tasks, completed or pending
	fn delete_job(job_id: &Uuid);
}

mod manager;
