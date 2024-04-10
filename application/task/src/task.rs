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

mod manager;
