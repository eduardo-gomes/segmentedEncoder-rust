use std::process::{exit, Command};
use std::str::FromStr;

use tonic::codegen::http::uri::PathAndQuery;
use tonic::codegen::InterceptedService;
use tonic::transport::{Channel, Endpoint, Uri};
use tonic::{Request, Status};
use uuid::Uuid;

use grpc_proto::proto::segmented_encoder_client::SegmentedEncoderClient;
use grpc_proto::proto::{client_with_auth, Empty, RegistrationRequest, Task};

#[tokio::main]
async fn main() {
	let args: Vec<String> = std::env::args().collect();
	if args.len() < 2 {
		println!("Missing arguments!\nUsage: {} SERVER_URL", args[0]);
		exit(1);
	} else {
		let url = Uri::from_str(&args[1]).expect("Failed to parse url");
		println!("Target url: {url}");
		let mut authenticated = connect(&url).await;
		let task = authenticated.request_task(Empty {}).await;
		let task = dbg!(task).expect("Did not get a task!");
		let command = task_to_command(&url, task.get_ref()).unwrap();
		println!("Command: {:?}", command);
	}
}

fn task_to_command(url: &Uri, task: &Task) -> Result<Command, Box<dyn std::error::Error>> {
	let id = task.id.as_ref().unwrap();
	let job_id = Uuid::from_slice(id.job_id.as_slice()).expect("Invalid UUID");
	let task_id = Uuid::from_slice(id.task_id.as_slice()).expect("Invalid UUID");
	let output_path = format!("/api/jobs/{job_id}/tasks/{task_id}/output");
	let output = {
		let mut output = url.clone().into_parts();
		output.path_and_query = Some(PathAndQuery::from_str(&output_path)?);
		Uri::from_parts(output)?
	};
	let input = {
		let mut input = url.clone().into_parts();
		input.path_and_query = Some(PathAndQuery::from_str(&task.input_path)?);
		Uri::from_parts(input)?
	};

	let v_args = shell_words::split(&task.v_params)?;
	let a_args = shell_words::split(&task.a_params)?;

	let mut command = Command::new("ffmpeg");
	command
		.arg("-i")
		.arg(input.to_string())
		.arg("-c:v")
		.arg(&task.v_codec)
		.args(v_args)
		.arg("-c:a")
		.arg(&task.a_codec)
		.args(a_args)
		.arg("-f")
		.arg("matroska")
		.arg(output.to_string());
	Ok(command)
}

async fn connect(
	url: &Uri,
) -> SegmentedEncoderClient<
	InterceptedService<Channel, impl Fn(Request<()>) -> Result<Request<()>, Status> + Sized>,
> {
	url.scheme()
		.expect("URL should have a scheme. Ex: https://localhost");
	let mut authenticated = connection_register(Endpoint::from(url.clone())).await;
	let response = authenticated
		.get_worker_registration(Empty {})
		.await
		.unwrap();
	dbg!(response.into_inner().parse().unwrap());
	authenticated
}

async fn connection_register(
	endpoint: Endpoint,
) -> SegmentedEncoderClient<
	InterceptedService<Channel, impl Fn(Request<()>) -> Result<Request<()>, Status>>,
> {
	let channel = endpoint.connect().await.unwrap();
	let mut connection = SegmentedEncoderClient::new(channel.clone());
	let auth = connection
		.register_client(RegistrationRequest {
			display_name: "Client".to_string(),
		})
		.await
		.unwrap();
	let worker_id = Uuid::from_slice(auth.into_inner().worker_id.as_slice()).unwrap();
	client_with_auth(channel, worker_id)
}
