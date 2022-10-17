pub mod proto {
	pub use helper::*;

	tonic::include_proto!("segmented_encoder");
	mod helper {
		use tonic::codegen::InterceptedService;
		use tonic::transport::Channel;
		use tonic::{Request, Status};
		use uuid::Uuid;

		use crate::proto::segmented_encoder_client::SegmentedEncoderClient;
		use crate::proto::RegistrationResponse;

		type SegmentedEncoderClientWithAuth<F> =
			SegmentedEncoderClient<InterceptedService<Channel, F>>;

		pub fn client_with_auth(
			channel: Channel,
			worker_id: Uuid,
		) -> SegmentedEncoderClientWithAuth<impl Fn(Request<()>) -> Result<Request<()>, Status>> {
			SegmentedEncoderClient::with_interceptor(channel, move |mut req: Request<()>| {
				req.metadata_mut()
					.insert("worker-id", worker_id.to_string().parse().unwrap());
				Ok(req)
			})
		}

		#[derive(Debug)]
		pub struct RegistrationResponseParsed {
			pub worker_id: Uuid,
		}

		impl RegistrationResponse {
			pub fn parse(self) -> Result<RegistrationResponseParsed, uuid::Error> {
				let worker_id = Uuid::from_slice(self.worker_id.as_slice())?;
				Ok(RegistrationResponseParsed { worker_id })
			}
		}
	}
}

pub mod echo {
	pub mod pb {
		tonic::include_proto!("echo");
	}

	pub mod client {
		use std::time::Duration;

		use futures_util::StreamExt;
		use tokio_stream::wrappers::IntervalStream;
		use tokio_stream::Stream;
		use tonic::transport::Channel;

		use crate::echo::pb::echo_client::EchoClient;
		use crate::echo::pb::EchoRequest;

		fn make_stream() -> impl Stream<Item = EchoRequest> {
			let interval = tokio::time::interval(Duration::from_secs(1));
			let interval_stream = IntervalStream::new(interval);
			tokio_stream::iter(0..usize::MAX)
				.zip(interval_stream)
				.map(|(i, _)| EchoRequest {
					message: format!("echo seq {i}"),
				})
		}

		pub async fn counting_client(url: &str) -> Result<(), Box<dyn std::error::Error>> {
			let endpoint = Channel::builder(url.parse()?);
			let mut client = EchoClient::connect(endpoint).await?;
			let req_stream = make_stream();
			let response = client.streaming_echo(req_stream).await?;
			let mut res_stream = response.into_inner();
			while let Some(response) = res_stream.next().await {
				let message = response?;
				println!("{message:?}");
			}
			Ok(())
		}
	}

	pub mod service {
		use std::pin::Pin;

		use futures_util::Stream;
		use tokio::sync::mpsc;
		use tokio_stream::{wrappers::ReceiverStream, StreamExt};
		use tonic::{Request, Response, Status, Streaming};
		use tower::make::Shared;

		use crate::echo::pb::echo_server::EchoServer;

		use super::pb::{EchoRequest, EchoResponse};

		#[derive(Clone)]
		pub struct EchoService {}

		pub fn shared() -> Shared<EchoServer<EchoService>> {
			Shared::new(EchoServer::new(EchoService {}))
		}

		type ResponseStream = Pin<Box<dyn Stream<Item = Result<EchoResponse, Status>> + Send>>;

		#[tonic::async_trait]
		impl super::pb::echo_server::Echo for EchoService {
			type StreamingEchoStream = ResponseStream;

			async fn streaming_echo(
				&self,
				request: Request<Streaming<EchoRequest>>,
			) -> Result<Response<Self::StreamingEchoStream>, Status> {
				let mut in_stream = request.into_inner();
				let (tx, rx) = mpsc::channel(16);

				// this spawn here is required if you want to handle connection error.
				// If we just map `in_stream` and write it back as `out_stream` the `out_stream`
				// will be drooped when connection error occurs and error will never be propagated
				// to mapped version of `in_stream`.
				tokio::spawn(async move {
					while let Some(result) = in_stream.next().await {
						match result {
							Ok(msg) => tx
								.send(Ok(EchoResponse {
									message: format!("Server got: {}", msg.message),
								}))
								.await
								.expect("working rx"),
							Err(err) => {
								match tx.send(Err(err)).await {
									Ok(_) => (),
									Err(_err) => break, // response was dropped
								}
							}
						}
					}
					println!("\tstream ended");
				});

				// echo just write the same data that was received
				let out_stream = ReceiverStream::new(rx);

				Ok(Response::new(
					Box::pin(out_stream) as Self::StreamingEchoStream
				))
			}
		}
	}
}
