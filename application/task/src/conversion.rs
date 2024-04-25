use api::models::{AnalysisTask, CodecParams, TaskRequestRecipe, TranscodeTask};

use super::*;

impl TryFrom<&api::models::Recipe> for Recipe {
	type Error = ();

	fn try_from(value: &api::models::Recipe) -> Result<Self, Self::Error> {
		let transcode = value.transcode.as_ref().map(|e| &e.options);
		match (&value.analysis, transcode, &value.merge) {
			(Some(s), None, None) => Ok(Recipe::Analysis(s.duration)),
			(None, Some(opt), None) => Ok(Recipe::Transcode(opt.clone())),
			(None, None, Some(_)) => Ok(Recipe::Merge(vec![])),
			(_, _, _) => Err(()),
		}
	}
}

impl TryFrom<api::models::TaskInputInner> for Input {
	type Error = ();

	fn try_from(value: api::models::TaskInputInner) -> Result<Self, Self::Error> {
		Ok(Input {
			index: u32::try_from(value.input).or(Err(()))?,
			start: value.start,
			end: value.end,
		})
	}
}

impl From<Input> for api::models::TaskInputInner {
	fn from(value: Input) -> Self {
		Self {
			input: value.index.try_into().unwrap_or(i32::MAX),
			start: value.start,
			end: value.end,
		}
	}
}

impl TryFrom<api::models::Task> for Instance {
	type Error = ();

	fn try_from(value: api::models::Task) -> Result<Self, Self::Error> {
		let job_id = Uuid::parse_str(&value.job_id).or(Err(()))?;
		let task_id = Uuid::parse_str(&value.task_id).or(Err(()))?;
		let inputs: Result<Vec<Input>, ()> = value.input.into_iter().map(Input::try_from).collect();
		let inputs = inputs?;
		let recipe = Recipe::try_from(value.recipe.as_ref())?;
		let job_options = value.job_options.as_ref().clone().into();
		Ok(Instance {
			job_id,
			task_id,
			inputs,
			recipe,
			job_options,
		})
	}
}

impl From<api::models::JobOptions> for JobOptions {
	fn from(value: api::models::JobOptions) -> Self {
		JobOptions {
			video: value.video.as_ref().clone().into(),
			audio: value.audio.map(|v| v.as_ref().clone().into()),
		}
	}
}

impl From<JobOptions> for api::models::JobOptions {
	fn from(value: JobOptions) -> Self {
		Self {
			video: Box::new(value.video.into()),
			audio: value.audio.map(|v| Box::new(v.clone().into())),
		}
	}
}

impl From<CodecParams> for Options {
	fn from(value: CodecParams) -> Self {
		Self {
			codec: value.codec,
			params: value.params.unwrap_or_default(),
		}
	}
}

impl From<Options> for CodecParams {
	fn from(value: Options) -> Self {
		Self {
			codec: value.codec,
			params: value.params.into(),
		}
	}
}

impl From<Recipe> for api::models::Recipe {
	fn from(value: Recipe) -> Self {
		match value {
			Recipe::Analysis(val) => api::models::Recipe {
				analysis: Some(Box::new(AnalysisTask { duration: val })),
				transcode: None,
				merge: None,
			},
			Recipe::Transcode(options) => api::models::Recipe {
				analysis: None,
				transcode: Some(Box::new(TranscodeTask { options })),
				merge: None,
			},
			Recipe::Merge(val) => api::models::Recipe {
				analysis: None,
				transcode: None,
				merge: Some(
					api::models::MergeTask {
						concatenate: val
							.into_iter()
							.map(|v| TryInto::<i32>::try_into(v).unwrap_or(i32::MAX))
							.collect(),
					}
					.into(),
				),
			},
		}
	}
}

impl From<Instance> for api::models::Task {
	fn from(value: Instance) -> api::models::Task {
		let job_id = value.job_id.to_string();
		let task_id = value.task_id.to_string();
		let input = value
			.inputs
			.into_iter()
			.map(api::models::TaskInputInner::from)
			.collect();
		let recipe = Box::new(value.recipe.into());
		let job_options = Box::new(value.job_options.into());
		api::models::Task {
			job_id,
			task_id,
			input,
			recipe,
			job_options,
		}
	}
}

impl From<api::models::TaskStatus> for Status {
	fn from(value: api::models::TaskStatus) -> Self {
		match value.successfully_completed {
			Some(true) => Status::Finished,
			_ => Status::Running,
		}
	}
}

impl From<Status> for api::models::TaskStatus {
	fn from(value: Status) -> Self {
		use api::models::TaskStatus;
		let finished = match value {
			Status::Finished => Some(true),
			Status::Running => None,
		};
		TaskStatus {
			successfully_completed: finished,
		}
	}
}

impl TryFrom<api::models::TaskRequest> for TaskSource {
	type Error = ();
	fn try_from(value: api::models::TaskRequest) -> Result<Self, Self::Error> {
		let recipe: Recipe = match *value.recipe {
			TaskRequestRecipe::TranscodeTask(task) => Recipe::Transcode(task.options),
			TaskRequestRecipe::MergeTask(task) => Recipe::Merge(
				task.concatenate
					.iter()
					.map(|v| (*v).try_into().unwrap_or(u32::MAX))
					.collect(),
			),
		};
		let inputs: Result<Vec<Input>, _> =
			value.inputs.into_iter().map(|v| v.try_into()).collect();
		let inputs = inputs?;
		Ok(TaskSource { inputs, recipe })
	}
}
