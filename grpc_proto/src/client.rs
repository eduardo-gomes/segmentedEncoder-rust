use tonic::Request;

use grpc_proto::status::status_reporter_client::StatusReporterClient;
use grpc_proto::status::Empty;

use self::status_reporter::DelayedStatusReporterStream;

mod status_reporter;

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
