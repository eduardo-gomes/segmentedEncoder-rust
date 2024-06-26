//! #Task crate
//! This crate defines the tasks, and includes the task runner under a feature, and the job/task manager trait

use uuid::Uuid;

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct JobSource {
	pub input_id: Uuid,
	pub options: JobOptions,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JobOptions {
	pub video: Options,
	pub audio: Option<Options>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskSource {
	///Here, the input should be the task id, or 0 for the job source
	pub inputs: Vec<Input>,
	pub recipe: Recipe,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
	pub codec: Option<String>,
	pub params: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Recipe {
	///Determines how long the tasks segments should be
	Analysis(Option<f64>),
	///Extra options for transcoding
	Transcode(Vec<String>),
	Merge(Vec<u32>),
}

#[derive(Clone)]
pub enum Status {
	Finished,
	Running,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Input {
	pub index: u32,
	pub start: Option<f64>,
	pub end: Option<f64>,
}

impl Input {
	pub fn source() -> Input {
		Input {
			index: 0,
			start: None,
			end: None,
		}
	}
}

///An allocated task
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
	pub job_id: Uuid,
	pub task_id: Uuid,
	pub inputs: Vec<Input>,
	pub recipe: Recipe,
	pub job_options: JobOptions,
}

mod conversion;

pub mod manager;
