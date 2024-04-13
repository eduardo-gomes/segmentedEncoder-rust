//! #Task crate
//! This crate defines the tasks, and includes the task runner under a feature, and the job/task manager trait

use uuid::Uuid;

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct JobSource {
	pub input_id: Uuid,
	pub video_options: Options,
}

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct TaskSource {
	///Here, the input should be the task id, or 0 for the job source
	pub inputs: Vec<Input>,
	pub recipe: Recipe,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
	pub codec: String,
	pub params: Vec<String>,
}

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub enum Recipe {
	///Determines how long the tasks segments should be
	Analysis(Option<f32>),
	Transcode(Options),
	Merge(Vec<u32>),
}

#[derive(Clone)]
pub enum Status {
	Finished,
	Running,
}

#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Input {
	index: u32,
	start: Option<f64>,
	end: Option<f64>,
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
#[derive(Clone)]
#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct Instance {
	pub job_id: Uuid,
	pub task_id: Uuid,
	pub inputs: Vec<Input>,
	pub recipe: Recipe,
}

mod conversion {
	use super::*;

	impl TryFrom<&api::models::Recipe> for Recipe {
		type Error = ();

		fn try_from(value: &api::models::Recipe) -> Result<Self, Self::Error> {
			let transcode = value.transcode.as_ref().map(|e| &e.options);
			match (&value.analysis, transcode, &value.merge) {
				(Some(s), None, None) => Ok(Recipe::Analysis(s.duration)),
				(None, Some(opt), None) => Ok(Recipe::Transcode(Options {
					codec: opt.codec.clone().unwrap_or_default(),
					params: opt.params.clone().unwrap_or_default(),
				})),
				(None, None, Some(_)) => Ok(Recipe::Merge(vec![])),
				(_, _, _) => Err(()),
			}
		}
	}

	impl TryFrom<api::models::Task> for Instance {
		type Error = ();

		fn try_from(value: api::models::Task) -> Result<Self, Self::Error> {
			let job_id = value
				.job_id
				.as_deref()
				.map(Uuid::parse_str)
				.transpose()
				.unwrap_or_default()
				.ok_or(())?;
			let task_id = value
				.task_id
				.as_deref()
				.map(Uuid::parse_str)
				.transpose()
				.unwrap_or_default()
				.ok_or(())?;
			let from_recipe = value.recipe.ok_or(())?;
			let recipe = Recipe::try_from(from_recipe.as_ref())?;
			Ok(Instance {
				job_id,
				task_id,
				inputs: vec![],
				recipe,
			})
		}
	}
}

pub mod manager;
