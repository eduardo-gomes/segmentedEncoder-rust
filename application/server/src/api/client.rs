use std::io::ErrorKind;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use uuid::Uuid;

use task::manager::Manager;

use crate::api::{AppState, AuthToken};
use crate::storage::Storage;

trait ClientApi: AppState {
	async fn get_job_output(&self, job_id: Uuid) -> Result<Uuid, (StatusCode, &'static str)> {
		self.manager()
			.get_job_output(&job_id)
			.await
			.map_err(|err| match err.kind() {
				ErrorKind::NotFound => (StatusCode::NOT_FOUND, "Job not found"),
				_ => (StatusCode::INTERNAL_SERVER_ERROR, "Server error"),
			})?
			.ok_or((StatusCode::SERVICE_UNAVAILABLE, "Output not available yet"))
	}

	async fn get_task_output(
		&self,
		job_id: Uuid,
		task_id: Uuid,
	) -> Result<Uuid, (StatusCode, &'static str)> {
		self.manager()
			.get_allocated_task_output(&job_id, &task_id)
			.await
			.map_err(|err| match err.kind() {
				ErrorKind::NotFound => (StatusCode::NOT_FOUND, "Job not found"),
				_ => (StatusCode::INTERNAL_SERVER_ERROR, "Server error"),
			})?
			.ok_or((StatusCode::SERVICE_UNAVAILABLE, "Output not available yet"))
	}
}

impl<T: AppState> ClientApi for T {}

pub(crate) async fn task_output_get<S: AppState>(
	State(state): State<Arc<S>>,
	_auth: AuthToken,
	Path((job_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, Response> {
	let stored = state
		.get_task_output(job_id, task_id)
		.await
		.map_err(|s| s.into_response())?;
	let read = state.storage().read_file(stored).await.or(Err((
		StatusCode::INTERNAL_SERVER_ERROR,
		"Invalid file",
	)
		.into_response()))?;
	crate::api::utils::ranged::from_reader(read, None)
		.await
		.or(Err(StatusCode::INTERNAL_SERVER_ERROR.into_response()))?
}

pub(super) async fn job_output_get<S: AppState>(
	State(state): State<Arc<S>>,
	_auth: AuthToken,
	Path(job_id): Path<Uuid>,
) -> Result<Response, Response> {
	let read = state
		.get_job_output(job_id)
		.await
		.map_err(|e| e.into_response())?;
	use crate::storage::Storage;
	let read = state
		.storage()
		.read_file(read)
		.await
		.or(Err(StatusCode::INTERNAL_SERVER_ERROR.into_response()))?;
	let ranged = crate::api::utils::ranged::from_reader(read, None)
		.await
		.or(Err(StatusCode::INTERNAL_SERVER_ERROR.into_response()))?;
	Ok(ranged.into_response())
}

pub(crate) async fn get_job_list<S: AppState>(
	State(state): State<Arc<S>>,
	_auth: AuthToken,
) -> Result<Json<Vec<Uuid>>, StatusCode> {
	state
		.manager()
		.get_job_list()
		.await
		.or(Err(StatusCode::INTERNAL_SERVER_ERROR))
		.map(Json)
}

#[cfg(test)]
mod test {
	use axum::http::StatusCode;
	use futures::AsyncWriteExt;
	use uuid::Uuid;

	use auth_module::LocalAuthenticator;
	use task::manager::{LocalJobManager, Manager};
	use task::Recipe::Transcode;
	use task::{Input, JobOptions, JobSource, Options, TaskSource};

	use crate::api::AppState;
	use crate::storage::{MemStorage, Storage};
	use crate::WEBM_SAMPLE;

	use super::super::worker::test_util::*;
	use super::ClientApi;

	#[tokio::test]
	async fn client_api_get_output_for_invalid_job_err_not_found() {
		let manager = LocalJobManager::default();
		let state = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: manager,
			_storage: MemStorage::default(),
		};
		let (code, _) = state
			.get_job_output(Uuid::nil())
			.await
			.expect_err("Should err for not found");
		assert_eq!(code, StatusCode::NOT_FOUND)
	}

	#[tokio::test]
	async fn client_api_get_output_before_is_available_503() {
		let manager = LocalJobManager::default();
		let job_id = manager
			.create_job(JobSource {
				input_id: Default::default(),
				options: JobOptions {
					video: Options {
						codec: None,
						params: vec![],
					},
					audio: None,
				},
			})
			.await
			.unwrap();
		let state = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: manager,
			_storage: MemStorage::default(),
		};
		let (code, _) = state
			.get_job_output(job_id)
			.await
			.expect_err("Should err for unavailable");
		assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE)
	}

	#[tokio::test]
	async fn client_api_get_output_return_content_uuid() {
		let output: Vec<u8> = WEBM_SAMPLE.iter().cloned().chain(0..123).collect();
		let storage = MemStorage::default();
		let mut write = storage.create_file().await.unwrap();
		write.write_all(output.as_slice()).await.unwrap();
		let file = storage.store_file(write).await.unwrap();

		let manager = LocalJobManager::default();
		let job_id = manager
			.create_job(JobSource {
				input_id: Default::default(),
				options: JobOptions {
					video: Options {
						codec: None,
						params: vec![],
					},
					audio: None,
				},
			})
			.await
			.unwrap();
		manager
			.add_task_to_job(
				&job_id,
				TaskSource {
					inputs: vec![Input::source()],
					recipe: Transcode(Vec::new()),
				},
			)
			.await
			.unwrap();
		let allocated = manager.allocate_task().await.unwrap().unwrap();
		manager
			.set_task_output(&allocated.job_id, &allocated.task_id, file)
			.await
			.unwrap()
			.expect("Should set");
		let state = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: manager,
			_storage: MemStorage::default(),
		};
		let file_id = state.get_job_output(job_id).await.expect("Job has output");
		assert_eq!(file_id, file)
	}

	#[tokio::test]
	async fn get_task_output_invalid_task_err_not_found() {
		let manager = LocalJobManager::default();
		let state = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: manager,
			_storage: MemStorage::default(),
		};
		let (code, _) = state
			.get_task_output(Uuid::nil(), Uuid::nil())
			.await
			.expect_err("Should err for not found");
		assert_eq!(code, StatusCode::NOT_FOUND)
	}

	#[tokio::test]
	async fn client_api_get_task_output_before_is_available_503() {
		let manager = LocalJobManager::default();
		let state = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: manager,
			_storage: MemStorage::default(),
		};
		let job_id = state
			.manager()
			.create_job(JobSource {
				input_id: Default::default(),
				options: JobOptions {
					video: Options {
						codec: None,
						params: vec![],
					},
					audio: None,
				},
			})
			.await
			.unwrap();
		state
			.manager()
			.add_task_to_job(
				&job_id,
				TaskSource {
					inputs: vec![Input::source()],
					recipe: Transcode(Vec::new()),
				},
			)
			.await
			.unwrap();
		let allocated = state.manager().allocate_task().await.unwrap().unwrap();
		let (code, _) = state
			.get_task_output(allocated.job_id, allocated.task_id)
			.await
			.expect_err("Should err for unavailable");
		assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE)
	}

	#[tokio::test]
	async fn client_api_get_task_output_return_content_uuid() {
		let output: Vec<u8> = WEBM_SAMPLE.iter().cloned().chain(0..123).collect();
		let storage = MemStorage::default();
		let mut write = storage.create_file().await.unwrap();
		write.write_all(output.as_slice()).await.unwrap();
		let file = storage.store_file(write).await.unwrap();

		let manager = LocalJobManager::default();
		let job_id = manager
			.create_job(JobSource {
				input_id: Default::default(),
				options: JobOptions {
					video: Options {
						codec: None,
						params: vec![],
					},
					audio: None,
				},
			})
			.await
			.unwrap();
		manager
			.add_task_to_job(
				&job_id,
				TaskSource {
					inputs: vec![Input::source()],
					recipe: Transcode(Vec::new()),
				},
			)
			.await
			.unwrap();
		let allocated = manager.allocate_task().await.unwrap().unwrap();
		manager
			.set_task_output(&allocated.job_id, &allocated.task_id, file)
			.await
			.unwrap()
			.expect("Should set");
		let state = GenericApp {
			credential: "".to_string(),
			_auth_handler: LocalAuthenticator::default(),
			_manager: manager,
			_storage: MemStorage::default(),
		};
		let file_id = state
			.get_task_output(allocated.job_id, allocated.task_id)
			.await
			.expect("Task has output");
		assert_eq!(file_id, file)
	}
}

#[cfg(test)]
mod test_handle {
	use axum::http::header::AUTHORIZATION;
	use axum::http::StatusCode;
	use uuid::Uuid;

	use task::{JobOptions, JobSource, Options, Recipe, TaskSource};

	use crate::api::AppState;
	use crate::WEBM_SAMPLE;

	use super::super::worker::test_util::*;

	#[tokio::test]
	async fn get_task_output_without_auth_forbidden() {
		let server = test_server();
		let code = server
			.get(&format!("/job/{}/task/{}/output", Uuid::nil(), Uuid::nil()))
			.await
			.status_code();
		assert_eq!(code, StatusCode::FORBIDDEN)
	}

	#[tokio::test]
	async fn get_task_output_with_auth_bad_task_not_found() {
		let (server, auth) = test_server_auth().await;
		let code = server
			.get(&format!("/job/{}/task/{}/output", Uuid::nil(), Uuid::nil()))
			.add_header(AUTHORIZATION, auth)
			.await
			.status_code();
		assert_eq!(code, StatusCode::NOT_FOUND)
	}

	#[tokio::test]
	async fn get_task_output_with_auth_invalid_task_bad_request() {
		let (server, auth) = test_server_auth().await;
		let code = server
			.get("/job/BAD/task/BAD/output")
			.add_header(AUTHORIZATION, auth)
			.await
			.status_code();
		assert_eq!(code, StatusCode::BAD_REQUEST)
	}

	#[tokio::test]
	async fn get_task_output_unfinished_unavailable() {
		let (server, app, auth) = test_server_state_auth().await;
		use task::manager::Manager;
		let job_id = app
			.manager()
			.create_job(JobSource {
				input_id: Default::default(),
				options: JobOptions {
					video: Options {
						codec: None,
						params: vec![],
					},
					audio: None,
				},
			})
			.await
			.unwrap();
		app.manager()
			.add_task_to_job(
				&job_id,
				TaskSource {
					inputs: vec![],
					recipe: Recipe::Transcode(Vec::new()),
				},
			)
			.await
			.unwrap();
		let instance = app.manager().allocate_task().await.unwrap().unwrap();
		let code = server
			.get(&format!(
				"/job/{}/task/{}/output",
				instance.job_id, instance.task_id
			))
			.add_header(AUTHORIZATION, auth)
			.await
			.status_code();
		assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE)
	}

	#[tokio::test]
	async fn get_task_output_returns_task_output() {
		let (server, app, auth) = test_server_state_auth().await;
		use task::manager::Manager;
		let job_id = app
			.manager()
			.create_job(JobSource {
				input_id: Default::default(),
				options: JobOptions {
					video: Options {
						codec: None,
						params: vec![],
					},
					audio: None,
				},
			})
			.await
			.unwrap();
		app.manager()
			.add_task_to_job(
				&job_id,
				TaskSource {
					inputs: vec![],
					recipe: Recipe::Transcode(Vec::new()),
				},
			)
			.await
			.unwrap();
		let instance = app.manager().allocate_task().await.unwrap().unwrap();
		let content: Vec<u8> = WEBM_SAMPLE.iter().cloned().chain(32..98).collect();
		let output = {
			use crate::storage::Storage;
			let mut file = app.storage().create_file().await.unwrap();
			use tokio::io::AsyncWriteExt;
			file.write_all(content.as_slice()).await.unwrap();
			app.storage().store_file(file).await.unwrap()
		};
		app.manager()
			.set_task_output(&job_id, &instance.task_id, output)
			.await
			.unwrap()
			.unwrap();
		let res = server
			.get(&format!(
				"/job/{}/task/{}/output",
				instance.job_id, instance.task_id
			))
			.add_header(AUTHORIZATION, auth)
			.await
			.into_bytes()
			.to_vec();
		assert_eq!(res, content)
	}

	#[tokio::test]
	async fn list_jobs_requires_auth() {
		let server = test_server();
		let res = server.get("/job").await;
		assert_eq!(res.status_code(), StatusCode::FORBIDDEN)
	}

	#[tokio::test]
	async fn list_jobs_success_with_auth() {
		let (server, auth) = test_server_auth().await;
		let res = server.get("/job").add_header(AUTHORIZATION, auth).await;
		assert!(res.status_code().is_success())
	}

	#[tokio::test]
	async fn list_jobs_returns_json_array() {
		let (server, auth) = test_server_auth().await;
		let _array = server
			.get("/job")
			.add_header(AUTHORIZATION, auth)
			.await
			.json::<Vec<Uuid>>();
	}

	#[tokio::test]
	async fn list_jobs_returns_json_array_with_the_created_job_id() {
		let (server, app, auth) = test_server_state_auth().await;
		use task::manager::Manager;
		let id = app
			.manager()
			.create_job(JobSource {
				input_id: Default::default(),
				options: JobOptions {
					video: Options {
						codec: None,
						params: vec![],
					},
					audio: None,
				},
			})
			.await
			.unwrap();
		let array = server
			.get("/job")
			.add_header(AUTHORIZATION, auth)
			.await
			.json::<Vec<Uuid>>();
		assert!(array.contains(&id))
	}

	mod job_output {
		use super::*;

		#[tokio::test]
		async fn get_without_auth_forbidden() {
			let server = test_server();
			let code = server
				.get(&format!("/job/{}/output", Uuid::nil()))
				.await
				.status_code();
			assert_eq!(code, StatusCode::FORBIDDEN)
		}

		#[tokio::test]
		async fn get_with_auth_bad_job_not_found() {
			let (server, auth) = test_server_auth().await;
			let code = server
				.get(&format!("/job/{}/output", Uuid::nil()))
				.add_header(AUTHORIZATION, auth)
				.await
				.status_code();
			assert_eq!(code, StatusCode::NOT_FOUND)
		}

		#[tokio::test]
		async fn get_with_auth_invalid_job_bad_request() {
			let (server, auth) = test_server_auth().await;
			let code = server
				.get("/job/BAD/output")
				.add_header(AUTHORIZATION, auth)
				.await
				.status_code();
			assert_eq!(code, StatusCode::BAD_REQUEST)
		}

		#[tokio::test]
		async fn get_unfinished_unavailable() {
			let (server, app, auth) = test_server_state_auth().await;
			use task::manager::Manager;
			let job_id = app
				.manager()
				.create_job(JobSource {
					input_id: Default::default(),
					options: JobOptions {
						video: Options {
							codec: None,
							params: vec![],
						},
						audio: None,
					},
				})
				.await
				.unwrap();
			app.manager()
				.add_task_to_job(
					&job_id,
					TaskSource {
						inputs: vec![],
						recipe: Recipe::Transcode(Vec::new()),
					},
				)
				.await
				.unwrap();
			let instance = app.manager().allocate_task().await.unwrap().unwrap();
			let code = server
				.get(&format!("/job/{}/output", instance.job_id))
				.add_header(AUTHORIZATION, auth)
				.await
				.status_code();
			assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE)
		}

		#[tokio::test]
		async fn get_returns_task_output() {
			let (server, app, auth) = test_server_state_auth().await;
			use task::manager::Manager;
			let job_id = app
				.manager()
				.create_job(JobSource {
					input_id: Default::default(),
					options: JobOptions {
						video: Options {
							codec: None,
							params: vec![],
						},
						audio: None,
					},
				})
				.await
				.unwrap();
			app.manager()
				.add_task_to_job(
					&job_id,
					TaskSource {
						inputs: vec![],
						recipe: Recipe::Transcode(Vec::new()),
					},
				)
				.await
				.unwrap();
			let instance = app.manager().allocate_task().await.unwrap().unwrap();
			let content: Vec<u8> = WEBM_SAMPLE.iter().cloned().chain(32..98).collect();
			let output = {
				use crate::storage::Storage;
				let mut file = app.storage().create_file().await.unwrap();
				use tokio::io::AsyncWriteExt;
				file.write_all(content.as_slice()).await.unwrap();
				app.storage().store_file(file).await.unwrap()
			};
			app.manager()
				.set_task_output(&job_id, &instance.task_id, output)
				.await
				.unwrap()
				.unwrap();
			let res = server
				.get(&format!("/job/{}/output", instance.job_id))
				.add_header(AUTHORIZATION, auth)
				.await
				.into_bytes()
				.to_vec();
			assert_eq!(res, content)
		}
	}
}
