//! #Task crate
//! This crate defines the tasks, and includes the task runner under a feature, and the job/task manager trait

use uuid::Uuid;

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct JobSource {
	input_id: Uuid,
	video_options: Options,
}

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct TaskSource {
	///Here, the input should be the task id, or 0 for the job source
	inputs: Vec<Input>,
	recipe: Recipe,
}

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
struct Options {
	codec: String,
	params: Vec<String>,
}

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
enum Recipe {
	Analysis(),
	Transcode(Options),
	Merge(Vec<u32>),
}

#[derive(Clone)]
enum Status {
	Finished,
	Running,
}

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
struct Input {
	index: u32,
	start: Option<f64>,
	end: Option<f64>,
}

///An allocated task
#[cfg_attr(test, derive(Debug, PartialEq))]
struct Instance {
	job_id: Uuid,
	task_id: Uuid,
	inputs: Vec<Input>,
	recipe: Recipe,
}

pub mod manager;
