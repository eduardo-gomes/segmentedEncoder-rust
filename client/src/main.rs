use std::process::exit;

use tonic::codegen::InterceptedService;
use tonic::transport::{Channel, Endpoint};
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
		let url = &args[1];
		println!("Target is {url}");
		let mut authenticated = connection_register(url.parse().unwrap()).await;
		let response = authenticated
			.get_worker_registration(Empty {})
			.await
			.unwrap();
		dbg!(response.into_inner().parse().unwrap());
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
