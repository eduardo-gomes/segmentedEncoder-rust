use api::models::AnalysisTask;

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
		let inputs: Result<Vec<Input>, ()> = value
			.input
			.ok_or(())?
			.into_iter()
			.map(Input::try_from)
			.collect();
		let inputs = inputs?;
		let recipe = Recipe::try_from(from_recipe.as_ref())?;
		Ok(Instance {
			job_id,
			task_id,
			inputs,
			recipe,
		})
	}
}

impl From<Options> for api::models::TranscodeTask {
	fn from(value: Options) -> Self {
		Self {
			options: Box::new(api::models::CodecParams {
				codec: Some(value.codec),
				params: Some(value.params),
			}),
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
			Recipe::Transcode(opt) => api::models::Recipe {
				analysis: None,
				transcode: Some(Box::new(opt.into())),
				merge: None,
			},
			Recipe::Merge(_) => api::models::Recipe {
				analysis: None,
				transcode: None,
				merge: Some(Default::default()),
			},
		}
	}
}

impl From<Instance> for api::models::Task {
	fn from(value: Instance) -> api::models::Task {
		let job_id = Some(value.job_id.to_string());
		let task_id = Some(value.task_id.to_string());
		let input = Some(
			value
				.inputs
				.into_iter()
				.map(|input| api::models::TaskInputInner {
					input: input.index as i32,
					start: input.start,
					end: input.end,
				})
				.collect(),
		);
		let recipe = Some(Box::new(value.recipe.into()));
		api::models::Task {
			job_id,
			task_id,
			input,
			recipe,
		}
	}
}