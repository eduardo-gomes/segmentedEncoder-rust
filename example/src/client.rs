use tonic::Request;

use status::status_reporter_client::StatusReporterClient;
use status::Empty;

pub mod status {
	tonic::include_proto!("status"); // The string specified here must match the proto package name
}

mod status_reporter;
use status_reporter::DelayedStatusReporterStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let mut client = StatusReporterClient::connect("http://[::1]:8888").await?;

	let response = client.get_latest_report(Request::new(Empty {})).await;

	match response {
		Ok(res) => println!("Server latest report: {:?}", res.get_ref()),
		Err(err) => println!("Could not get latest report from server:\n\t{err:#}"),
	}

	let report_stream = DelayedStatusReporterStream::new(100, 32);
	let request = Request::new(report_stream);

	let response = client.report(request).await?;

	println!("Server response to status: {:?}", response.get_ref());

	let response = client.get_latest_report(Request::new(Empty {})).await?;

	println!("Server latest report: {:?}", response.get_ref());
	Ok(())
}
