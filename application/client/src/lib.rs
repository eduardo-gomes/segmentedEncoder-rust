use std::process::Command;

use uuid::Uuid;

use api::apis::configuration::Configuration;
use task::{Input, Instance, Options, Recipe, Status, TaskSource};

#[allow(async_fn_in_trait)]
pub trait TaskRunner {
	fn get_input_url(&self, job: Uuid, task: Uuid, idx: u32) -> String;
	fn get_output_url(&self, job: Uuid, task: Uuid) -> String;
	fn get_input_creds(&self) -> String;
	fn get_output_creds(&self) -> String {
		self.get_input_creds()
	}
	async fn mark_task_complete(&self, job: Uuid, task: Uuid) -> Result<(), ()>;

	async fn add_task_to_job(&self, job: Uuid, task: TaskSource) -> Result<(), ()>;

	async fn run_analysis(&self, task: Instance, option: Option<f64>) -> Result<(), ()> {
		let source = TaskSource {
			inputs: vec![Input::source()],
			recipe: Recipe::Transcode(Options {
				codec: "libx264".to_string(),
				params: ["-preset", "ultrafast", "-crf", "30"]
					.into_iter()
					.map(String::from)
					.collect(),
			}),
		};
		self.add_task_to_job(task.job_id, source).await
	}
	async fn run_transcode(&self, task: Instance, recipe: Options) -> Result<(), ()> {
		let inputs = task
			.inputs
			.into_iter()
			.flat_map(|input| {
				let source = [
					"-headers".to_string(),
					format!("Authorization: {}", self.get_input_creds()),
					"-i".to_string(),
					self.get_input_url(task.job_id, task.task_id, input.index),
				];
				let start = input
					.start
					.map(|start| ["-ss".to_string(), start.to_string()]);
				let end = input.end.map(|end| ["-to".to_string(), end.to_string()]);
				let args: Vec<String> = start
					.into_iter()
					.flatten()
					.chain(end.into_iter().flatten())
					.chain(source.into_iter())
					.collect();
				args
			})
			.collect::<Vec<_>>();
		let codec = ["-c:v", recipe.codec.as_str()]
			.into_iter()
			.map(String::from);
		let params = recipe.params.into_iter();
		let output = [
			"-f".to_string(),
			"matroska".to_string(),
			"-method".to_string(),
			"PUT".to_string(),
			"-headers".to_string(),
			format!("Authorization: {}", self.get_output_creds()),
			self.get_output_url(task.job_id, task.task_id),
		];
		let mut ffmpeg = Command::new("ffmpeg");
		ffmpeg
			.args(inputs.into_iter())
			.args(codec)
			.args(params)
			.args(output.into_iter());
		println!("Command: {:?}", ffmpeg);
		let status = ffmpeg
			.status()
			.expect("Failed to run ffmpeg")
			.code()
			.unwrap();
		println!("ffmpeg returned: {status}");
		let res = self.mark_task_complete(task.job_id, task.task_id).await;
		println!("Mark task complete: {:?}", res);
		Ok(())
	}

	async fn run(&self, task: Instance) {
		let _ = match task.recipe.clone() {
			Recipe::Analysis(analysis) => self.run_analysis(task, analysis).await,
			Recipe::Transcode(transcode) => self.run_transcode(task, transcode).await,
			Recipe::Merge(_) => unimplemented!("Merge task is not implemented"),
		};
	}
}

impl TaskRunner for Configuration {
	fn get_input_url(&self, job: Uuid, task: Uuid, idx: u32) -> String {
		format!("{}/job/{}/task/{}/input/{}", self.base_path, job, task, idx)
	}

	fn get_output_url(&self, job: Uuid, task: Uuid) -> String {
		format!("{}/job/{}/task/{}/output", self.base_path, job, task)
	}

	fn get_input_creds(&self) -> String {
		self.api_key
			.as_ref()
			.map(|k| k.key.to_string())
			.unwrap_or_default()
	}

	async fn mark_task_complete(&self, job: Uuid, task: Uuid) -> Result<(), ()> {
		let res = api::apis::worker_api::job_job_id_task_task_id_status_post(
			self,
			&job.to_string(),
			&task.to_string(),
			Some(Status::Finished.into()),
		)
		.await;
		res.or(Err(()))
	}

	async fn add_task_to_job(&self, job: Uuid, task: TaskSource) -> Result<(), ()> {
		let recipe = match task.recipe {
			Recipe::Transcode(t) => Some(t),
			_ => None,
		}
		.ok_or(())?;
		let parsed = api::models::TaskRequest {
			inputs: vec![Input::source().into()],
			recipe: Box::new(api::models::TaskRequestRecipe::TranscodeTask(Box::new(
				recipe.into(),
			))),
		};
		api::apis::worker_api::job_job_id_task_post(self, &job.to_string(), Some(parsed))
			.await
			.or(Err(()))
			.and(Ok(()))
	}
}
