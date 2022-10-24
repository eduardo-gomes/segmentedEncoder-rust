use std::process::exit;
use std::str::FromStr;

use tonic::codegen::InterceptedService;
use tonic::transport::{Channel, Endpoint, Uri};
use tonic::{Request, Status};

use grpc_proto::proto::segmented_encoder_client::SegmentedEncoderClient;
use grpc_proto::proto::{client_with_auth, Empty, RegistrationRequest};

#[tokio::main]
async fn main() {
	let args: Vec<String> = std::env::args().collect();
	if args.len() < 2 {
		println!("Missing arguments!\nUsage: {} SERVER_URL", args[0]);
		exit(1);
	} else {
		let url = Uri::from_str(&args[1]).expect("Failed to parse url");
		println!("Target url: {url}");
		url.scheme()
			.expect("URL should have a scheme. Ex: https://localhost");
		let mut authenticated = connection_register(Endpoint::from(url)).await;
		let response = authenticated
			.get_worker_registration(Empty {})
			.await
			.unwrap();
		dbg!(response.into_inner().parse().unwrap());
		let task = authenticated.request_task(Empty {}).await;
		dbg!(task).expect("Did not get a task!");
	}
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
	let worker_id = uuid::Uuid::from_slice(auth.into_inner().worker_id.as_slice()).unwrap();
	client_with_auth(channel, worker_id)
}
