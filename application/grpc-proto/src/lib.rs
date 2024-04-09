pub mod proto {
	pub use helper::*;

	tonic::include_proto!("segmented_encoder");
	mod helper {
		use tonic::codegen::InterceptedService;
		use tonic::transport::Channel;
		use tonic::{IntoRequest, Request, Status};
		use uuid::Uuid;

		use crate::proto::segmented_encoder_client::SegmentedEncoderClient;
		use crate::proto::{Empty, RegistrationResponse};

		pub type SegmentedEncoderClientWithAuth<F> =
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

		impl IntoRequest<Empty> for () {
			fn into_request(self) -> Request<Empty> {
				Request::new(Empty {})
			}
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
