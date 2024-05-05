//! Worker api
//!
//! Define the routes used by the workers to execute tasks

use std::io::ErrorKind;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::headers::Range;
use axum_extra::TypedHeader;
use tokio::io::{AsyncRead, AsyncSeek};
use uuid::Uuid;

use task::manager::Manager;
use task::TaskSource;

use crate::api::utils::ranged::from_reader;
use crate::api::{AppState, AuthToken};
use crate::storage::Storage;

trait WorkerApi {
	async fn allocate_task(&self) -> Result<Json<api::models::Task>, impl IntoResponse>;
	async fn get_task_input_file(
		&self,
		job_id: Uuid,
		task_id: Uuid,
		input_idx: u32,
	) -> Result<impl AsyncRead + AsyncSeek + Send + Unpin + 'static, StatusCode>;
	///Append task to job and returns the task number
	async fn append_task_to_job(
		&self,
		job_id: Uuid,
		source: api::models::TaskRequest,
	) -> Result<u32, StatusCode>;
}

impl<T: AppState> WorkerApi for T {
	async fn allocate_task(&self) -> Result<Json<api::models::Task>, StatusCode> {
		self.manager()
			.allocate_task()
			.await
			.map(|opt| opt.map(|val| Json(val.into())))
			.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
			.ok_or(StatusCode::SERVICE_UNAVAILABLE)
	}

	async fn get_task_input_file(
		&self,
		job_id: Uuid,
		task_id: Uuid,
		input_idx: u32,
	) -> Result<impl AsyncRead + AsyncSeek + Send + Unpin + 'static, StatusCode> {
		let file = self
			.manager()
			.get_allocated_task_input(&job_id, &task_id, input_idx)
			.await
			.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
			.ok_or(StatusCode::NOT_FOUND)?;
		self.storage()
			.read_file(file)
			.await
			.or(Err(StatusCode::INTERNAL_SERVER_ERROR))
	}

	async fn append_task_to_job(
		&self,
		job_id: Uuid,
		source: api::models::TaskRequest,
	) -> Result<u32, StatusCode> {
		let task: TaskSource = source.try_into().or(Err(StatusCode::BAD_REQUEST))?;
		self.manager()
			.add_task_to_job(&job_id, task)
			.await
			.map(Some)
			.or_else(|err| match err.kind() {
				ErrorKind::NotFound => Ok(None),
				ErrorKind::InvalidInput => Err(StatusCode::BAD_REQUEST),
				_ => Err(StatusCode::INTERNAL_SERVER_ERROR),
			})
			.and_then(|v| v.ok_or(StatusCode::NOT_FOUND))
	}
}

pub(super) async fn allocate_task<S: AppState>(
	State(state): State<Arc<S>>,
	_auth: AuthToken,
) -> Result<Json<api::models::Task>, StatusCode> {
	state.allocate_task().await
}

pub(super) async fn get_task_input<S: AppState>(
	State(state): State<Arc<S>>,
	_auth: AuthToken,
	range: Option<TypedHeader<Range>>,
	Path((job_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, StatusCode> {
	let read = state.get_task_input_file(job_id, task_id, 0).await?;
	let ranged = from_reader(read, range.map(|TypedHeader(r)| r))
		.await
		.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
	Ok(ranged.into_response())
}

pub(super) async fn put_task_output<S: AppState>(
	State(state): State<Arc<S>>,
	_auth: AuthToken,
	Path((job_id, task_id)): Path<(Uuid, Uuid)>,
	body: Body,
) -> Result<StatusCode, StatusCode> {
	state
		.manager()
		.get_task(&job_id, &task_id)
		.await
		.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?
		.ok_or(StatusCode::NOT_FOUND)?;
	let file = state
		.storage()
		.body_to_new_file(body)
		.await
		.or(Err(StatusCode::INTERNAL_SERVER_ERROR))?;
	state
		.manager()
		.set_task_output(&job_id, &task_id, file)
		.await
		.and(Ok(StatusCode::ACCEPTED))
		.or(Err(StatusCode::INTERNAL_SERVER_ERROR))
}

pub(super) async fn task_status_post<S: AppState>(
	State(state): State<Arc<S>>,
	_auth: AuthToken,
	Path((job_id, task_id)): Path<(Uuid, Uuid)>,
	Json(body): Json<api::models::TaskStatus>,
) -> StatusCode {
	let res = state
		.manager()
		.update_task_status(&job_id, &task_id, body.into())
		.await;
	match res {
		Ok(Some(_)) => StatusCode::NO_CONTENT,
		Ok(None) => StatusCode::NOT_FOUND,
		Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
	}
}

pub(super) async fn task_post<S: AppState>(
	State(state): State<Arc<S>>,
	_auth: AuthToken,
	Path(job_id): Path<Uuid>,
	Json(request): Json<api::models::TaskRequest>,
) -> Result<(StatusCode, String), StatusCode> {
	let task: TaskSource = request
		.try_into()
		.or(Err(StatusCode::UNPROCESSABLE_ENTITY))?;
	let result = state.manager().add_task_to_job(&job_id, task).await;
	let idx = result.map_err(|e| match e.kind() {
		ErrorKind::NotFound => StatusCode::NOT_FOUND,
		_ => StatusCode::INTERNAL_SERVER_ERROR,
	})?;
	Ok((StatusCode::CREATED, idx.to_string()))
}

#[cfg(test)]
pub(crate) mod test_util {
	use std::future::Future;
	use std::io::Error;
	use std::sync::Arc;

	use axum::http::HeaderValue;
	use axum_test::TestServer;
	use uuid::Uuid;

	use auth_module::AuthenticationHandler;
	use task::manager::Manager;
	use task::{Input, Instance, JobOptions, JobSource, Options, Recipe, Status, TaskSource};

	use crate::api::AppState;
	use crate::storage::Storage;
	use crate::{AppStateLocal, WEBM_SAMPLE};

	pub(crate) use super::super::test::*;

	mockall::mock! {
	pub ThisManager{}
	impl Manager for ThisManager{
			fn create_job(&self, job: JobSource) -> impl Future<Output=Result<Uuid, Error>> + Send;

			fn get_job(&self, job_id: &Uuid) -> impl Future<Output=Result<Option<JobSource>, Error>> + Send;

			fn get_job_list(&self) -> impl Future<Output=Result<Vec<Uuid>, Error>> + Send;

			fn allocate_task(&self) -> impl Future<Output=Result<Option<Instance>, Error>> + Send;

			fn add_task_to_job(&self, job_id: &Uuid, task: TaskSource) -> impl Future<Output=Result<u32, Error>> + Send;

			fn get_task_source(&self, job_id: &Uuid, task_idx: u32) -> impl Future<Output=Result<Option<TaskSource>, Error>> + Send;

			fn get_task(&self, job_id: &Uuid, task_id: &Uuid) -> impl Future<Output=Result<Option<Instance>, Error>> + Send;

			fn update_task_status(&self, job_id: &Uuid, task_id: &Uuid, status: Status) -> impl Future<Output=Result<Option<()>, Error>> + Send;

			fn set_task_output(&self, job_id: &Uuid, task_id: &Uuid, output: Uuid) -> impl Future<Output=Result<Option<()>, Error>> + Send;

			fn get_task_output(&self, job_id: &Uuid, task_idx: u32) -> impl Future<Output=Result<Option<Uuid>, Error>> + Send;

			fn get_allocated_task_output(&self, job_id: &Uuid, task_id: &Uuid) -> impl Future<Output=Result<Option<Uuid>, Error>> + Send;

			fn get_allocated_task_input(&self, job_id: &Uuid, task_id: &Uuid, input_idx: u32) -> impl Future<Output = Result<Option<Uuid>, Error>> + Send;

			fn get_job_output(&self, job_id: &Uuid) -> impl Future<Output=Result<Option<Uuid>, Error>> + Send;

			fn cancel_task(&self, job_id: &Uuid, task_id: &Uuid) -> impl Future<Output=Result<Option<()>, Error>> + Send;

			fn delete_job(&self, job_id: &Uuid) -> impl Future<Output=Result<Option<()>, Error>> + Send;

		}
	}

	pub struct GenericApp<A: AuthenticationHandler, B: Manager, C: Storage> {
		pub credential: String,
		pub _auth_handler: A,
		pub _manager: B,
		pub _storage: C,
	}

	impl<
			A: AuthenticationHandler + Sync + Send,
			B: Manager + Sync + Send,
			C: Storage + Sync + Send,
		> AppState for GenericApp<A, B, C>
	{
		fn manager(&self) -> &impl Manager {
			&self._manager
		}

		fn auth_handler(&self) -> &impl AuthenticationHandler {
			&self._auth_handler
		}

		fn storage(&self) -> &impl Storage {
			&self._storage
		}

		fn check_credential(&self, cred: &str) -> bool {
			self.credential == cred
		}
	}

	pub(crate) async fn app_with_job_and_analyse_task(
	) -> (TestServer, Arc<AppStateLocal>, HeaderValue) {
		let app = AppStateLocal::default();
		let data = axum::body::Body::from(WEBM_SAMPLE.as_slice());
		let input = app._storage.body_to_new_file(data).await.unwrap();
		let job = create_job_source(input);
		let job_id = app._manager.create_job(job).await.unwrap();
		app._manager
			.add_task_to_job(
				&job_id,
				TaskSource {
					inputs: vec![Input::source()],
					recipe: Recipe::Analysis(None),
				},
			)
			.await
			.unwrap();
		test_server_state_auth_generic(Arc::new(app)).await
	}

	pub(crate) fn create_job_options() -> JobOptions {
		JobOptions {
			video: Options {
				codec: Some("libx264".to_string()),
				params: vec![],
			},
			audio: None,
		}
	}

	pub(crate) fn create_job_source(input_id: Uuid) -> JobSource {
		JobSource {
			input_id,
			options: create_job_options(),
		}
	}

	pub struct MergeRecipe(pub Vec<i32>);

	impl From<MergeRecipe> for api::models::TaskRequestRecipe {
		fn from(value: MergeRecipe) -> Self {
			let concatenate = value.0;
			api::models::TaskRequestRecipe::MergeTask(Box::new(api::models::MergeTask {
				concatenate,
			}))
		}
	}
}

#[cfg(test)]
mod test_allocate_task {
	use std::sync::Arc;

	use axum::http::header::AUTHORIZATION;
	use axum::http::StatusCode;
	use uuid::Uuid;

	use auth_module::LocalAuthenticator;
	use task::{Input, Instance, Recipe};

	use crate::storage::MemStorage;

	use super::test_util::*;

	#[tokio::test]
	async fn requires_auth() {
		let (server, _, _) = test_server_state_auth().await;
		let res = server.get("/allocate_task").await.status_code();
		assert_eq!(res, StatusCode::FORBIDDEN)
	}

	#[tokio::test]
	async fn with_auth_will_probe_manager() {
		let mut mock_manager = MockThisManager::new();
		mock_manager
			.expect_allocate_task()
			.times(1)
			.returning(|| Box::pin(async { Ok(None) }));
		let state = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: mock_manager,
			_storage: MemStorage::default(),
		};
		let (server, _, auth) = test_server_state_auth_generic(Arc::new(state)).await;
		server
			.get("/allocate_task")
			.add_header(AUTHORIZATION, auth)
			.await
			.assert_status_not_ok();
	}

	#[tokio::test]
	async fn without_auth_will_not_probe_manager() {
		let mut mock_manager = MockThisManager::new();
		mock_manager.expect_allocate_task().never();
		let state = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: mock_manager,
			_storage: MemStorage::default(),
		};
		let (server, _, _) = test_server_state_auth_generic(Arc::new(state)).await;
		let code = server.get("/allocate_task").await.status_code();
		assert_eq!(code, StatusCode::FORBIDDEN)
	}

	#[tokio::test]
	async fn will_return_value_from_manager() {
		let mut mock_manager = MockThisManager::new();
		let instance = Instance {
			job_id: Uuid::from_u64_pair(1, 2),
			task_id: Uuid::from_u64_pair(1, 3),
			inputs: vec![Input::source()],
			recipe: Recipe::Analysis(None),
			job_options: create_job_options(),
		};
		let _result = instance.clone();
		mock_manager
			.expect_allocate_task()
			.times(1)
			.returning(move || {
				let _result = _result.clone();
				Box::pin(async { Ok(Some(_result)) })
			});
		let state = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: mock_manager,
			_storage: MemStorage::default(),
		};
		let (server, _, auth) = test_server_state_auth_generic(Arc::new(state)).await;
		let res = server
			.get("/allocate_task")
			.add_header(AUTHORIZATION, auth)
			.await;
		assert!(res.status_code().is_success());
		let got: Instance = res.json::<api::models::Task>().try_into().unwrap();
		assert_eq!(got, instance);
	}

	#[tokio::test]
	async fn without_task_available_returns_unavailable() {
		let (server, _, auth) = test_server_state_auth().await;
		let code = server
			.get("/allocate_task")
			.add_header(AUTHORIZATION, auth)
			.await
			.status_code();
		assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
	}
}

#[cfg(test)]
mod test_get_input {
	use axum::http::header::{AUTHORIZATION, RANGE};
	use axum::http::{HeaderValue, StatusCode};
	use tokio::io::AsyncReadExt;
	use uuid::Uuid;

	use task::manager::Manager;

	use crate::api::test::{test_server, test_server_auth};
	use crate::api::AppState;
	use crate::storage::Storage;

	use super::test_util::*;

	#[tokio::test]
	async fn requires_authentication() {
		let server = test_server();
		let path = format!(
			"/job/{id}/task/{id}/input/0",
			id = Uuid::nil().as_hyphenated()
		);
		let code = server.get(&path).await.status_code();
		assert_eq!(code, StatusCode::FORBIDDEN)
	}

	#[tokio::test]
	async fn with_no_job_returns_not_found() {
		let (server, auth) = test_server_auth().await;
		let path = format!("/job/{id}/task/{id}/input/0", id = Uuid::nil());
		let code = server
			.get(&path)
			.add_header(AUTHORIZATION, auth)
			.await
			.status_code();
		assert_eq!(code, StatusCode::NOT_FOUND)
	}

	#[tokio::test]
	async fn with_non_uuid_task_id_bad_request() {
		let (server, auth) = test_server_auth().await;
		let uuid = Uuid::nil();
		let path = format!("/job/{uuid}/task/BAD_UUID/input/0");
		let code = server
			.get(&path)
			.add_header(AUTHORIZATION, auth)
			.await
			.status_code();
		assert_eq!(code, StatusCode::BAD_REQUEST)
	}

	#[tokio::test]
	async fn with_non_uuid_job_id_bad_request() {
		let (server, auth) = test_server_auth().await;
		let uuid = Uuid::nil();
		let path = format!("/job/BAD_UUID/task/{uuid}/input/0");
		let code = server
			.get(&path)
			.add_header(AUTHORIZATION, auth)
			.await
			.status_code();
		assert_eq!(code, StatusCode::BAD_REQUEST)
	}

	#[tokio::test]
	async fn with_valid_task_is_success() {
		let (server, app, auth) = app_with_job_and_analyse_task().await;
		let task = app
			.manager()
			.allocate_task()
			.await
			.unwrap()
			.expect("There should be a task");
		assert!(!task.inputs.is_empty(), "This task should have a input");
		let path = format!("/job/{}/task/{}/input/0", task.job_id, task.task_id);
		let code = server
			.get(&path)
			.add_header(AUTHORIZATION, auth)
			.await
			.status_code();
		assert!(code.is_success())
	}

	#[tokio::test]
	async fn returns_the_right_content_on_the_body() {
		let (server, app, auth) = app_with_job_and_analyse_task().await;
		let task = app
			.manager()
			.allocate_task()
			.await
			.unwrap()
			.expect("There should be a task");
		assert!(!task.inputs.is_empty(), "This task should have a input");
		let input_id = app
			.manager()
			.get_job(&task.job_id)
			.await
			.unwrap()
			.unwrap()
			.input_id;
		let path = format!("/job/{}/task/{}/input/0", task.job_id, task.task_id);
		let ret = server
			.get(&path)
			.add_header(AUTHORIZATION, auth)
			.await
			.into_bytes()
			.to_vec();
		let mut expected = Vec::new();
		app.storage()
			.read_file(input_id)
			.await
			.unwrap()
			.read_to_end(&mut expected)
			.await
			.unwrap();
		assert_eq!(ret, expected)
	}

	#[tokio::test]
	async fn range_returns_partial_content() {
		let (server, app, auth) = app_with_job_and_analyse_task().await;
		let task = app
			.manager()
			.allocate_task()
			.await
			.unwrap()
			.expect("There should be a task");
		assert!(!task.inputs.is_empty(), "This task should have a input");
		let input_id = app
			.manager()
			.get_job(&task.job_id)
			.await
			.unwrap()
			.unwrap()
			.input_id;
		let path = format!("/job/{}/task/{}/input/0", task.job_id, task.task_id);
		let response = server
			.get(&path)
			.add_header(AUTHORIZATION, auth)
			.add_header(RANGE, HeaderValue::from_static("bytes=0-10"))
			.await;
		let code = response.status_code();
		assert_eq!(code, StatusCode::PARTIAL_CONTENT);
	}

	#[tokio::test]
	async fn range_returns_partial_content_with_selected_range() {
		let (server, app, auth) = app_with_job_and_analyse_task().await;
		let task = app
			.manager()
			.allocate_task()
			.await
			.unwrap()
			.expect("There should be a task");
		assert!(!task.inputs.is_empty(), "This task should have a input");
		let input_id = app
			.manager()
			.get_job(&task.job_id)
			.await
			.unwrap()
			.unwrap()
			.input_id;
		let path = format!("/job/{}/task/{}/input/0", task.job_id, task.task_id);
		let range = 0..10 + 1;
		let ret = server
			.get(&path)
			.add_header(AUTHORIZATION, auth)
			.add_header(RANGE, HeaderValue::from_static("bytes=0-10"))
			.await
			.into_bytes()
			.to_vec();
		let mut expected = Vec::new();
		app.storage()
			.read_file(input_id)
			.await
			.unwrap()
			.read_to_end(&mut expected)
			.await
			.unwrap();
		assert_eq!(ret, &expected[range])
	}
}

#[cfg(test)]
mod test_post_input {
	use std::sync::Arc;

	use axum::http::header::AUTHORIZATION;
	use axum::http::{HeaderValue, StatusCode};
	use axum_test::TestServer;
	use tokio::io::AsyncReadExt;
	use uuid::Uuid;

	use auth_module::LocalAuthenticator;
	use task::Status;

	use crate::api::test::{test_server, test_server_auth, test_server_state_auth_generic};
	use crate::api::worker::test_util::GenericApp;
	use crate::api::worker::test_util::MockThisManager;
	use crate::api::AppState;
	use crate::storage::{MemStorage, Storage};
	use crate::WEBM_SAMPLE;

	#[tokio::test]
	async fn fail_without_auth() {
		let server = test_server();
		let path = format!("/job/{id}/task/{id}/output", id = Uuid::nil());
		let code = server.put(&path).await.status_code();
		assert_eq!(code, StatusCode::FORBIDDEN)
	}

	#[tokio::test]
	async fn with_auth_but_no_job_not_found() {
		let (server, auth) = test_server_auth().await;
		let path = format!("/job/{id}/task/{id}/output", id = Uuid::nil());
		let code = server
			.put(&path)
			.add_header(AUTHORIZATION, auth)
			.await
			.status_code();
		assert_eq!(code, StatusCode::NOT_FOUND)
	}

	#[tokio::test]
	async fn for_allocated_task_success() {
		use task::manager::Manager;
		let (server, app, auth) = super::test_util::app_with_job_and_analyse_task().await;
		let instance = app
			.manager()
			.allocate_task()
			.await
			.unwrap()
			.expect("Should have task");
		let path = format!("/job/{}/task/{}/output", instance.job_id, instance.task_id);
		let code = server
			.put(&path)
			.add_header(AUTHORIZATION, auth)
			.bytes(WEBM_SAMPLE.as_slice().into())
			.await
			.status_code();
		assert!(code.is_success())
	}

	async fn put_task_output(
		server: &TestServer,
		job_id: &Uuid,
		task_id: &Uuid,
		auth: HeaderValue,
		content: &'static [u8],
	) -> StatusCode {
		let path = format!("/job/{}/task/{}/output", job_id, task_id);
		server
			.put(&path)
			.add_header(AUTHORIZATION, auth)
			.bytes(content.into())
			.await
			.status_code()
	}
	#[tokio::test]
	async fn task_will_have_output_after_put() {
		use task::manager::Manager;
		let (server, app, auth) = super::test_util::app_with_job_and_analyse_task().await;
		let instance = app
			.manager()
			.allocate_task()
			.await
			.unwrap()
			.expect("Should have task");
		let source = WEBM_SAMPLE.as_slice();
		let put = put_task_output(&server, &instance.job_id, &instance.task_id, auth, source).await;
		assert!(put.is_success());
		let task_output = app
			.manager()
			.get_task_output(&instance.job_id, 0)
			.await
			.unwrap();
		assert!(task_output.is_some())
	}

	#[tokio::test]
	async fn will_store_the_content_on_storage() {
		use task::manager::Manager;
		let (server, app, auth) = super::test_util::app_with_job_and_analyse_task().await;
		let instance = app
			.manager()
			.allocate_task()
			.await
			.unwrap()
			.expect("Should have task");
		const SOURCE: &[u8] = WEBM_SAMPLE.as_slice();
		let put = put_task_output(&server, &instance.job_id, &instance.task_id, auth, SOURCE).await;
		assert!(put.is_success());
		let task_output = app
			.manager()
			.get_task_output(&instance.job_id, 0)
			.await
			.unwrap()
			.unwrap();
		let mut content = Vec::new();
		app.storage()
			.read_file(task_output)
			.await
			.unwrap()
			.read_to_end(&mut content)
			.await
			.unwrap();
		assert_eq!(content.as_slice(), SOURCE)
	}

	#[tokio::test]
	async fn status_post_returns_forbidden_without_auth() {
		use task::manager::Manager;
		let (server, app, _auth) = super::test_util::app_with_job_and_analyse_task().await;
		let instance = app
			.manager()
			.allocate_task()
			.await
			.unwrap()
			.expect("Should have task");
		let path = format!("/job/{}/task/{}/status", instance.job_id, instance.task_id);
		let code = server.post(&path).json("body").await.status_code();
		assert_eq!(code, StatusCode::FORBIDDEN)
	}

	#[tokio::test]
	async fn status_post_with_auth_not_forbidden() {
		use task::manager::Manager;
		let (server, app, auth) = super::test_util::app_with_job_and_analyse_task().await;
		let instance = app
			.manager()
			.allocate_task()
			.await
			.unwrap()
			.expect("Should have task");
		let path = format!("/job/{}/task/{}/status", instance.job_id, instance.task_id);
		let code = server
			.post(&path)
			.add_header(AUTHORIZATION, auth)
			.json("body")
			.await
			.status_code();
		assert_ne!(code, StatusCode::FORBIDDEN)
	}

	#[tokio::test]
	async fn status_post_complete_once_success() {
		let mut mock_manager = MockThisManager::new();
		mock_manager
			.expect_update_task_status()
			.withf(|_job, _task, status| matches!(status, Status::Finished))
			.times(1)
			.returning(|_job, _task, _status| Box::pin(async { Ok(None) }));
		let state = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: mock_manager,
			_storage: MemStorage::default(),
		};
		let (server, _, auth) = test_server_state_auth_generic(Arc::new(state)).await;
		let path = format!("/job/{}/task/{}/status", Uuid::nil(), Uuid::nil());
		let code = server
			.post(&path)
			.add_header(AUTHORIZATION, auth)
			.json(&Into::<api::models::TaskStatus>::into(Status::Finished))
			.await
			.status_code();
		assert_ne!(code, StatusCode::FORBIDDEN)
	}

	#[tokio::test]
	async fn status_post_with_bad_task_not_found() {
		let (server, _, auth) = super::test_util::app_with_job_and_analyse_task().await;
		let path = format!("/job/{}/task/{}/status", Uuid::nil(), Uuid::nil());
		let code = server
			.post(&path)
			.add_header(AUTHORIZATION, auth)
			.json(&Into::<api::models::TaskStatus>::into(Status::Finished))
			.await
			.status_code();
		assert_eq!(code, StatusCode::NOT_FOUND)
	}
}

#[cfg(test)]
mod test_task_post {
	use std::sync::Arc;

	use axum::http::header::AUTHORIZATION;
	use axum::http::StatusCode;
	use uuid::Uuid;

	use auth_module::LocalAuthenticator;
	use task::manager::Manager;
	use task::{Input, JobSource, TaskSource};

	use crate::api::test::{
		test_server, test_server_auth, test_server_state_auth, test_server_state_auth_generic,
	};
	use crate::api::worker::test_util::{
		create_job_options, GenericApp, MergeRecipe, MockThisManager,
	};
	use crate::api::worker::WorkerApi;
	use crate::api::AppState;
	use crate::storage::MemStorage;
	use crate::AppStateLocal;

	#[tokio::test]
	async fn append_task_to_non_existent_err() {
		let app = AppStateLocal::default();
		let task = api::models::TaskRequest {
			inputs: vec![Input::source().into()],
			recipe: Box::new(MergeRecipe(vec![0]).into()),
		};
		let err = app.append_task_to_job(Uuid::nil(), task).await;
		assert_eq!(err.unwrap_err(), StatusCode::NOT_FOUND);
	}

	#[tokio::test]
	async fn append_task_to_existent_job() {
		let app = AppStateLocal::default();
		let job = app
			.manager()
			.create_job(JobSource {
				input_id: Default::default(),
				options: create_job_options(),
			})
			.await
			.unwrap();
		let task = api::models::TaskRequest {
			inputs: vec![Input::source().into()],
			recipe: Box::new(MergeRecipe(vec![0]).into()),
		};
		let res = app.append_task_to_job(job, task).await;
		assert!(res.is_ok());
	}

	#[tokio::test]
	async fn append_multiple_task_returns_different_idx() {
		let app = AppStateLocal::default();
		let job = app
			.manager()
			.create_job(JobSource {
				input_id: Default::default(),
				options: create_job_options(),
			})
			.await
			.unwrap();
		let task = api::models::TaskRequest {
			inputs: vec![Input::source().into()],
			recipe: Box::new(MergeRecipe(vec![0]).into()),
		};
		let id_1 = app.append_task_to_job(job, task.clone()).await.unwrap();
		let id_2 = app.append_task_to_job(job, task).await.unwrap();
		assert_ne!(id_1, id_2);
	}

	#[tokio::test]
	async fn append_returns_idx_returned_by_manager() {
		static NUM: u32 = 12345;
		let mut mock_manager = MockThisManager::new();
		mock_manager
			.expect_add_task_to_job()
			.times(1)
			.returning(|_job, _task| Box::pin(async { Ok(NUM) }));
		let app = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: mock_manager,
			_storage: MemStorage::default(),
		};
		let task = api::models::TaskRequest {
			inputs: vec![Input::source().into()],
			recipe: Box::new(MergeRecipe(vec![0]).into()),
		};
		let id = app.append_task_to_job(Uuid::nil(), task).await.unwrap();
		assert_eq!(id, NUM);
	}

	#[tokio::test]
	async fn send_parsed_task_to_manager() {
		static NUM: u32 = 12345;
		let task = api::models::TaskRequest {
			inputs: vec![Input::source().into()],
			recipe: Box::new(MergeRecipe(vec![0]).into()),
		};
		let parsed: TaskSource = task.clone().try_into().unwrap();
		let mut mock_manager = MockThisManager::new();
		mock_manager
			.expect_add_task_to_job()
			.withf(move |_, task_parsed| parsed == *task_parsed)
			.times(1)
			.returning(|_job, _task| Box::pin(async { Ok(NUM) }));
		let app = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: mock_manager,
			_storage: MemStorage::default(),
		};
		let id = app.append_task_to_job(Uuid::nil(), task).await.unwrap();
		assert_eq!(id, NUM);
	}

	#[tokio::test]
	async fn endpoint_without_auth_forbidden() {
		let server = test_server();
		let res = server
			.post(&format!("/job/{}/task", Uuid::nil()))
			.await
			.status_code();
		assert_eq!(res, StatusCode::FORBIDDEN)
	}

	#[tokio::test]
	async fn endpoint_with_auth_not_forbidden() {
		let (server, auth) = test_server_auth().await;
		let res = server
			.post(&format!("/job/{}/task", Uuid::nil()))
			.add_header(AUTHORIZATION, auth)
			.await
			.status_code();
		assert_ne!(res, StatusCode::FORBIDDEN)
	}

	#[tokio::test]
	async fn endpoint_with_bad_body_is_client_error_other_than_not_found() {
		let (server, auth) = test_server_auth().await;
		let res = server
			.post(&format!("/job/{}/task", Uuid::nil()))
			.add_header(AUTHORIZATION, auth)
			.json("BAD BODY")
			.await
			.status_code();
		assert!(res.is_client_error());
		assert_ne!(res, StatusCode::NOT_FOUND)
	}

	#[tokio::test]
	async fn endpoint_with_good_body_invalid_job_error() {
		let (server, auth) = test_server_auth().await;
		let task = api::models::TaskRequest {
			inputs: vec![Input::source().into()],
			recipe: Box::new(MergeRecipe(vec![0, 1, 2]).into()),
		};
		let res = server
			.post(&format!("/job/{}/task", Uuid::nil()))
			.add_header(AUTHORIZATION, auth)
			.json(&task)
			.await
			.status_code();
		assert_eq!(res, StatusCode::NOT_FOUND)
	}

	#[tokio::test]
	async fn endpoint_with_invalid_uuid_is_bad_request() {
		let (server, auth) = test_server_auth().await;
		let task = api::models::TaskRequest {
			inputs: vec![Input::source().into()],
			recipe: Box::new(MergeRecipe(vec![0, 1, 2]).into()),
		};
		let res = server
			.post("/job/BAD_ID/task")
			.add_header(AUTHORIZATION, auth)
			.json(&task)
			.await
			.status_code();
		assert_eq!(res, StatusCode::BAD_REQUEST)
	}

	#[tokio::test]
	async fn endpoint_with_good_body_valid_job_created() {
		let (server, app, auth) = test_server_state_auth().await;
		let task = api::models::TaskRequest {
			inputs: vec![Input::source().into()],
			recipe: Box::new(MergeRecipe(vec![0, 1, 2]).into()),
		};
		let job_id = app
			.manager()
			.create_job(JobSource {
				input_id: Default::default(),
				options: create_job_options(),
			})
			.await
			.unwrap();
		let res = server
			.post(&format!("/job/{}/task", job_id))
			.add_header(AUTHORIZATION, auth)
			.json(&task)
			.await
			.status_code();
		assert_eq!(res, StatusCode::CREATED)
	}

	#[tokio::test]
	async fn endpoint_with_send_parsed_task_source_to_manager() {
		static NUM: u32 = 1;
		let job_id = Uuid::from_u64_pair(123, 456);
		let task = api::models::TaskRequest {
			inputs: vec![Input::source().into()],
			recipe: Box::new(MergeRecipe(vec![0]).into()),
		};
		let parsed: TaskSource = task.clone().try_into().unwrap();
		let mut mock_manager = MockThisManager::new();
		mock_manager
			.expect_add_task_to_job()
			.with(
				mockall::predicate::eq(job_id),
				mockall::predicate::eq(parsed),
			)
			.times(1)
			.returning(|_job, _task| Box::pin(async { Ok(NUM) }));
		let app = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: mock_manager,
			_storage: MemStorage::default(),
		};
		let (server, _, auth) = test_server_state_auth_generic(Arc::new(app)).await;
		let res = server
			.post(&format!("/job/{}/task", job_id))
			.add_header(AUTHORIZATION, auth)
			.json(&task)
			.await
			.status_code();
		assert!(res.is_success())
	}

	#[tokio::test]
	async fn endpoint_returns_the_created_task_idx() {
		static NUM: u32 = 1;
		let job_id = Uuid::from_u64_pair(123, 456);
		let task = api::models::TaskRequest {
			inputs: vec![Input::source().into()],
			recipe: Box::new(MergeRecipe(vec![0]).into()),
		};
		let parsed: TaskSource = task.clone().try_into().unwrap();
		let mut mock_manager = MockThisManager::new();
		mock_manager
			.expect_add_task_to_job()
			.times(1)
			.returning(|_job, _task| Box::pin(async { Ok(NUM) }));
		let app = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: mock_manager,
			_storage: MemStorage::default(),
		};
		let (server, _, auth) = test_server_state_auth_generic(Arc::new(app)).await;
		let idx = server
			.post(&format!("/job/{}/task", job_id))
			.add_header(AUTHORIZATION, auth)
			.json(&task)
			.await
			.json::<u32>();
		assert_eq!(idx, NUM)
	}
}
