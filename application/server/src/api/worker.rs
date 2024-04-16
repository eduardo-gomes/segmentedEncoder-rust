//! Worker api
//!
//! Define the routes used by the workers to execute tasks

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use task::manager::Manager;

use crate::api::{AppState, AuthToken};

pub(super) async fn allocate_task<S: AppState>(
	State(state): State<Arc<S>>,
	_auth: AuthToken,
) -> Result<Json<api::models::Task>, StatusCode> {
	let allocate = state
		.manager()
		.allocate_task()
		.await
		.unwrap()
		.ok_or(StatusCode::NOT_FOUND)?;
	Ok(Json(allocate.into()))
}

#[cfg(test)]
mod test {
	use std::future::Future;
	use std::io::Error;
	use std::sync::Arc;

	use axum::http::header::AUTHORIZATION;
	use axum::http::StatusCode;
	use uuid::Uuid;

	use auth_module::{AuthenticationHandler, LocalAuthenticator};
	use task::manager::Manager;
	use task::{Input, Instance, JobSource, Recipe, Status, TaskSource};

	use crate::api::AppState;
	use crate::storage::{MemStorage, Storage};

	use super::super::test::*;

	#[tokio::test]
	async fn allocate_task_requires_auth() {
		let (server, _, _) = test_server_state_auth().await;
		let res = server.get("/allocate_task").await.status_code();
		assert_eq!(res, StatusCode::FORBIDDEN)
	}

	mockall::mock! {
	ThisManager{}
	impl Manager for ThisManager{
			fn create_job(&self, job: JobSource) -> impl Future<Output=Result<Uuid, Error>> + Send;

			fn get_job(&self, job_id: &Uuid) -> impl Future<Output=Result<Option<JobSource>, Error>> + Send;

			fn allocate_task(&self) -> impl Future<Output=Result<Option<Instance>, Error>> + Send;

			fn add_task_to_job(&self, job_id: &Uuid, task: TaskSource) -> impl Future<Output=Result<u32, Error>> + Send;

			fn get_task_source(&self, job_id: &Uuid, task_idx: u32) -> impl Future<Output=Result<TaskSource, Error>> + Send;

			fn get_task(&self, job_id: &Uuid, task_id: &Uuid) -> impl Future<Output=Result<Option<Instance>, Error>> + Send;

			fn update_task_status(&self, job_id: &Uuid, task_id: &Uuid, status: Status) -> impl Future<Output=Result<(), Error>> + Send;

			fn set_task_output(&self, job_id: &Uuid, task_id: &Uuid, output: Uuid) -> impl Future<Output=Result<(), Error>> + Send;

			fn get_task_output(&self, job_id: &Uuid, task_idx: u32) -> impl Future<Output=Result<Option<Uuid>, Error>> + Send;

			fn cancel_task(&self, job_id: &Uuid, task_id: &Uuid) -> impl Future<Output=Result<(), Error>> + Send;

			fn delete_job(&self, job_id: &Uuid) -> impl Future<Output=Result<(), Error>> + Send;

		}
	}

	struct GenericApp<A: AuthenticationHandler, B: Manager, C: Storage> {
		credential: String,
		_auth_handler: A,
		_manager: B,
		_storage: C,
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

	#[tokio::test]
	async fn allocate_task_with_auth_will_probe_manager() {
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
	async fn allocate_task_without_auth_will_not_probe_manager() {
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
	async fn allocate_task_will_return_value_from_manager() {
		let mut mock_manager = MockThisManager::new();
		let instance = Instance {
			job_id: Uuid::from_u64_pair(1, 2),
			task_id: Uuid::from_u64_pair(1, 3),
			inputs: vec![Input::source()],
			recipe: Recipe::Analysis(None),
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
}
