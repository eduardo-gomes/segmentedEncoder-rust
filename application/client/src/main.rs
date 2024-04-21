use clap::Parser;

#[derive(Parser, Debug)]
#[command()]
struct Args {
	///Server api base url
	#[arg(short, long, default_value = "http://localhost:8888/api")]
	server: String,
	///Password to register worker with server
	#[arg(long, env = "CLIENT_PASSWORD")]
	password: String,
}

#[tokio::main]
async fn main() {
	let args = Args::parse();
	let base = args
		.server
		.parse::<reqwest::Url>()
		.expect("Should be valid uri");
	let config = api::apis::configuration::Configuration {
		base_path: args.server,
		..Default::default()
	};
	let server_version = api::apis::default_api::version_get(&config).await.unwrap();
	println!("Server: {}, version {:?}", base, server_version);
	let token = api::apis::default_api::login_get(&config, &args.password)
		.await
		.unwrap();
	println!("Login successful, token: {token}");
	unimplemented!("Client is not implemented, only try to login")
}
