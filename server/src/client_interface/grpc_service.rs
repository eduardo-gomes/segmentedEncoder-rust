use std::sync::Weak;

use tokio::sync::{RwLock, RwLockReadGuard};
use tonic::{Request, Response, Status};

use grpc_proto::proto::segmented_encoder_server::SegmentedEncoder;
use grpc_proto::proto::{Empty, RegistrationRequest, RegistrationResponse, Task};

use crate::client_interface::grpc_service::auth_interceptor::AuthenticationExtension;
use crate::State;

use super::Service;

pub(super) mod auth_interceptor;

/// A RwLock newType for [Service]. Used to implement [SegmentedEncoder]
pub struct ServiceLock(RwLock<Service>);

impl ServiceLock {
	pub(crate) fn with_state(self, state: Weak<State>) -> Self {
		self.0.into_inner().with_state(state).into_lock()
	}
	pub(crate) async fn read(&self) -> RwLockReadGuard<'_, Service> {
		self.0.read().await
	}
}

impl Service {
	///[ServiceLock] is a workaround to implement a trait from another crate
	pub(crate) fn into_lock(self) -> ServiceLock {
		ServiceLock(RwLock::new(self))
	}
}

#[tonic::async_trait]
impl SegmentedEncoder for ServiceLock {
	async fn register_client(
		&self,
		_request: Request<RegistrationRequest>,
	) -> Result<Response<RegistrationResponse>, Status> {
		let (uuid, _) = self.0.write().await.register_client();
		Ok(Response::new(RegistrationResponse {
			worker_id: uuid.into_bytes().to_vec(),
		}))
	}

	async fn get_worker_registration(
		&self,
		request: Request<Empty>,
	) -> Result<Response<RegistrationResponse>, Status> {
		let worker_id = AuthenticationExtension::verify_request(&request, self)
			.await
			.successful()?;
		Ok(Response::new(RegistrationResponse {
			worker_id: worker_id.into_bytes().to_vec(),
		}))
	}

	async fn request_task(&self, _request: Request<Empty>) -> Result<Response<Task>, Status> {
		let got = self
			.0
			.read()
			.await
			.request_task()
			.map_err(Status::unknown)?
			.await;
		got.ok_or_else(|| {
			Status::deadline_exceeded(
				"Deadline exceeded immediately because timeout was not implemented",
			)
		})
		.map(|task| {
			let params = task.parameters;
			Response::new(Task {
				input_path: task.input_path,
				v_codec: params.video_encoder.unwrap_or_default(),
				v_params: params.video_args.unwrap_or_default(),
				a_codec: params.audio_encoder.unwrap_or_default(),
				a_params: params.audio_args.unwrap_or_default(),
			})
		})
	}
}

#[cfg(test)]
mod test {
	use std::error::Error;
	use std::future::Future;
	use std::str::FromStr;
	use std::sync::Arc;

	use tokio::sync::RwLock;
	use tonic::transport::{Channel, Endpoint};
	use tonic::{Code, Request, Status};
	use tower::make::Shared;
	use uuid::Uuid;

	use grpc_proto::proto::segmented_encoder_client::SegmentedEncoderClient;
	use grpc_proto::proto::{client_with_auth, Empty, RegistrationRequest};

	use crate::client_interface::grpc_service::ServiceLock;
	use crate::client_interface::Service;
	use crate::storage::FileRef;
	use crate::State;

	#[tokio::test]
	async fn register_client_returns_uuid() -> Result<(), Box<dyn Error>> {
		let (close, mut client, _) = start_server(None).await?;

		let request = Request::new(RegistrationRequest {
			display_name: "Test worker".to_string(),
		});

		let response = client.register_client(request).await?;
		let worker_id = response.into_inner().worker_id;
		let worker_id = Uuid::from_slice(&worker_id).expect("worker_id is not an uuid");
		assert!(!worker_id.is_nil(), "worker_id should not be null");

		//shutdown server
		close.await?;
		Ok(())
	}

	#[tokio::test]
	async fn get_worker_registration_needs_authentication() -> Result<(), Box<dyn Error>> {
		let (close, mut client, _) = start_server(None).await?;

		let response = client
			.get_worker_registration(Empty {})
			.await
			.expect_err("Should not accept unauthenticated requests");
		assert_eq!(response.code(), Code::Unauthenticated);

		close.await?;
		Ok(())
	}

	#[tokio::test]
	async fn get_worker_registration_authenticated() -> Result<(), Box<dyn Error>> {
		let (close, mut client, url) = start_server(None).await?;
		let worker_id = register(&mut client).await?;
		let channel = Endpoint::from_str(&url)?.connect().await?;
		let mut client = client_with_auth(channel, worker_id);

		let response = client
			.get_worker_registration(Empty {})
			.await
			.expect("Should allow authenticated");
		let response = response.into_inner();
		assert_eq!(Uuid::from_slice(response.worker_id.as_slice())?, worker_id);

		close.await?;
		Ok(())
	}

	#[tokio::test]
	async fn random_uuid_is_not_authenticated() -> Result<(), Box<dyn Error>> {
		let (close, _, url) = start_server(None).await?;
		let channel = Endpoint::from_str(&url)?.connect().await?;
		let mut client =
			SegmentedEncoderClient::with_interceptor(channel, move |mut req: Request<()>| {
				req.metadata_mut()
					.insert("worker-id", Uuid::new_v4().to_string().parse().unwrap());
				Ok(req)
			});

		let response = client
			.get_worker_registration(Empty {})
			.await
			.expect_err("Should not authenticate");
		assert_eq!(response.code(), Code::Unauthenticated);

		close.await?;
		Ok(())
	}

	#[tokio::test]
	async fn request_task_before_create_task_should_timeout() -> Result<(), Box<dyn Error>> {
		let state = new_state();

		let (close, client, url) = start_server(Some(state)).await?;
		let (mut client, _) = register_connect(client, &url).await?;

		let err = client
			.request_task(())
			.await
			.expect_err("Should return fail");
		assert_eq!(err.code(), Code::DeadlineExceeded, "Status: {:?}", err);
		close.await
	}

	#[tokio::test]
	async fn request_task_after_create_task_return_task() -> Result<(), Box<dyn Error>> {
		use crate::jobs::{Job, JobParams, Source};
		let state = new_state();
		let job = Job::new(Source::File(FileRef::fake()), JobParams::sample_params());
		state.manager.write().await.add_job(job);

		let (close, client, url) = start_server(Some(state)).await?;
		let (mut client, _) = register_connect(client, &url).await?;

		let _task = client
			.request_task(())
			.await
			.expect("Should return after a job is created");

		close.await
	}

	#[tokio::test]
	async fn request_task_check_task_params() -> Result<(), Box<dyn Error>> {
		use crate::jobs::{Job, JobParams, Source};
		let state = new_state();
		let params = JobParams::sample_params();
		let job = Job::new(Source::File(FileRef::fake()), params.clone());
		state.manager.write().await.add_job(job);

		let (close, client, url) = start_server(Some(state)).await?;
		let (mut client, _) = register_connect(client, &url).await?;

		let task = client
			.request_task(())
			.await
			.expect("Should return after a job is created")
			.into_inner();
		let has_same_parameters = {
			let params = params.clone();
			task.a_codec == params.audio_encoder.unwrap_or_default()
				&& task.a_params == params.audio_args.unwrap_or_default()
				&& task.v_codec == params.video_encoder.unwrap_or_default()
				&& task.v_params == params.video_args.unwrap_or_default()
		};
		assert!(
			has_same_parameters,
			"Task: {:?}\nParams: {:?}\nBoth should have same parameters\n",
			task, params
		);
		close.await
	}

	#[tokio::test]
	async fn request_task_has_valid_input_path() -> Result<(), Box<dyn Error>> {
		use crate::jobs::{Job, JobParams, Source};
		let state = new_state();
		let params = JobParams::sample_params();
		let job = Job::new(Source::File(FileRef::fake()), params.clone());
		state.manager.write().await.add_job(job);

		let (close, client, url) = start_server(Some(state)).await?;
		let (mut client, _) = register_connect(client, &url).await?;

		let task = client
			.request_task(())
			.await
			.expect("Should return after a job is created")
			.into_inner();
		let path = task.input_path;
		//Append path to server url
		let input = reqwest::Url::parse(&url)?.join(&path)?;
		println!("Input: {}", input);
		let response = reqwest::get(input.clone())
			.await
			.expect("Task should have a valid input");
		assert!(
			response.status().is_success(),
			"Request to {} was not successful {}.",
			input,
			response.status()
		);
		close.await
	}

	fn new_state() -> Arc<State> {
		use crate::job_manager::JobManager;
		use crate::storage::Storage;
		let manager = JobManager::new(Storage::new().unwrap()).into();
		State::new(manager, Service::new().into_lock())
	}

	async fn register_connect(
		mut client: SegmentedEncoderClient<Channel>,
		url: &str,
	) -> Result<
		(
			grpc_proto::proto::SegmentedEncoderClientWithAuth<
				impl Fn(Request<()>) -> Result<Request<()>, Status>,
			>,
			Uuid,
		),
		Box<dyn Error>,
	> {
		let worker_id = register(&mut client).await?;
		let channel = Endpoint::from_str(url)?.connect().await?;
		let client = client_with_auth(channel, worker_id);

		Ok((client, worker_id))
	}

	//Request registration and return the worker_id
	async fn register(
		client: &mut SegmentedEncoderClient<Channel>,
	) -> Result<Uuid, Box<dyn Error>> {
		let request = Request::new(RegistrationRequest {
			display_name: "Test worker".to_string(),
		});
		let response = client.register_client(request).await?;
		let worker_id = response.into_inner().worker_id;
		let worker_id = Uuid::from_slice(&worker_id)?;
		Ok(worker_id)
	}

	//Start the server and return future to close, a client and the url
	async fn start_server(
		state: Option<Arc<State>>,
	) -> Result<
		(
			impl Future<Output = Result<(), Box<dyn Error>>>,
			SegmentedEncoderClient<Channel>,
			String,
		),
		Box<dyn Error>,
	> {
		let instance = state.clone().map_or_else(
			|| Arc::new(ServiceLock(RwLock::new(Service::new()))),
			|state| state.grpc.clone(),
		);
		let service = Shared::new(instance.with_auth());
		let addr = "[::1]:0".parse().unwrap();
		let server = hyper::Server::bind(&addr).serve(service);
		let addr = server.local_addr();
		let (tx, rx) = tokio::sync::oneshot::channel::<()>();
		let graceful = server.with_graceful_shutdown(async {
			rx.await.ok();
		});
		let server_handle = tokio::spawn(graceful);
		let port = addr.port();
		let url = format!("http://[::1]:{port}");

		let close = async move {
			tx.send(()).map_err(|_| "the receiver dropped")?;
			server_handle.await??;
			drop(state); //Make close own State while server is open
			Ok(())
		};
		let client = grpc_proto::proto::segmented_encoder_client::SegmentedEncoderClient::connect(
			url.clone(),
		)
		.await?;
		Ok((close, client, url))
	}
}
