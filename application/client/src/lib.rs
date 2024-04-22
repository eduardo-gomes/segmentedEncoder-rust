use std::process::Command;

use uuid::Uuid;

use api::apis::configuration::Configuration;
use task::Instance;

#[allow(async_fn_in_trait)]
pub trait TaskRunner {
	fn get_input_url(&self, job: Uuid, task: Uuid, idx: u32) -> String;
	fn get_output_url(&self, job: Uuid, task: Uuid) -> String;
	fn get_input_creds(&self) -> String;
	fn get_output_creds(&self) -> String {
		self.get_input_creds()
	}

	async fn run(&self, task: Instance) {
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
		ffmpeg.args(inputs.into_iter()).args(output.into_iter());
		println!("Command: {:?}", ffmpeg);
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
}
