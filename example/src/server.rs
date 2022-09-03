use tonic::transport::Server;

use status::status_reporter_server::StatusReporterServer;

pub mod status {
	tonic::include_proto!("status");
}

pub mod grpc {
	use futures_util::StreamExt; //stream.next()
	use std::sync::{Arc, RwLock};
	use tonic::{Request, Response, Status};

	use super::status::status_reporter_server::StatusReporter;
	use super::status::{Empty, ReportResult, StatusReport};

	pub struct StatusKeeper {
		latest: Arc<RwLock<Option<StatusReport>>>,
	}
	impl StatusKeeper {
		pub fn new() -> Self {
			StatusKeeper {
				latest: Arc::new(RwLock::new(None)),
			}
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let addr = "[::1]:8888".parse().unwrap();

	let status_reporter_service = grpc::StatusKeeper::new();

	let svc = StatusReporterServer::new(status_reporter_service);

	println!("Starting server on {:?}", addr);
	Server::builder().add_service(svc).serve(addr).await?;

	Ok(())
}
