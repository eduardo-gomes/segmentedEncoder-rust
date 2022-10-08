use std::process::exit;

#[tokio::main]
async fn main() {
	let args: Vec<String> = std::env::args().collect();
	if args.len() < 2 {
		println!("Missing arguments!\nUsage: {} SERVER_URL", args[0]);
		exit(1);
	} else {
		let url = &args[1];
		println!("Target is {url}");
		grpc_proto::echo::client::counting_client(url)
			.await
			.unwrap();
	}
}
