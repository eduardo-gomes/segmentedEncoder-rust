use std::io::ErrorKind;

use axum::http::StatusCode;
use uuid::Uuid;

use task::manager::Manager;

use crate::api::AppState;

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
