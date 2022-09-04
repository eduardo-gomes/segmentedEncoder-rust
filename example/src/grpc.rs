use futures_util::StreamExt; //stream.next()
use std::sync::{Arc, RwLock};
use tonic::{Request, Response, Status};

use super::status::status_reporter_server::StatusReporter;
use super::status::{Empty, ReportResult, StatusReport};

#[derive(Clone)]
pub struct StatusKeeper {
	latest: Arc<RwLock<Option<StatusReport>>>,
}
impl StatusKeeper {
	pub fn new() -> Self {
		StatusKeeper {
			latest: Arc::new(RwLock::new(None)),
		}
	}
	pub fn get_latest_report(&self) -> Option<StatusReport> {
		//This ignores errors, done to just implement http api
		self.latest.read().map(|val| val.clone()).unwrap_or(None)
	}
}

#[tonic::async_trait]
impl StatusReporter for StatusKeeper {
	async fn report(
		&self,
		request: Request<tonic::Streaming<StatusReport>>,
	) -> Result<Response<ReportResult>, Status> {
		let mut stream = request.into_inner();

		while let Some(report) = stream.next().await {
			let report = report?;
			println!("Received report: {:#?}", report);
			let mut latest = self
				.latest
				.write()
				.map_err(|poison| Status::internal(poison.to_string()))?;
			*latest = Some(report);
		}
		Ok(Response::new(ReportResult {}))
	}
	async fn get_latest_report(
		&self,
		_request: Request<Empty>,
	) -> Result<Response<StatusReport>, Status> {
		let report = self
			.latest
			.read()
			.map_err(|poison| Status::internal(poison.to_string()))?
			.to_owned();
		match report {
			Some(report) => Ok(Response::new(report)),
			None => Err(Status::unavailable("No report available")),
		}
	}
}
