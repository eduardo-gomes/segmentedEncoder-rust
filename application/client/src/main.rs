use std::time::Duration;

use clap::Parser;

use api::apis::configuration::ApiKey;
use api::apis::Error;
use client::TaskRunner;
use task::Instance;

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

async fn run_task(config: &api::apis::configuration::Configuration, task: Instance) {
	println!("Task: {:#?}", task);
	config.run(task).await;
	unimplemented!("Client cant run task yet");
}

async fn work_loop(config: &api::apis::configuration::Configuration) -> bool {
	println!("Requesting task...");
	let api_task = api::apis::worker_api::allocate_task_get(config).await;
	match api_task {
		Err(Error::ResponseError(e)) => {
			if 503 == e.status.as_u16() {
				println!("No tasks available");
				tokio::time::sleep(Duration::from_secs(5)).await;
				true
			} else {
				eprintln!("Unexpected error: {:?}", e);
				false
			}
		}
		Ok(api_task) => {
			match Instance::try_from(api_task) {
				Ok(task) => run_task(config, task).await,
				Err(e) => eprintln!("Failed to parse task: {e:?}"),
			}
			true
		}
		Err(e) => {
			eprintln!("Could not finish request: {:?}", e);
			false
		}
	}
}

#[tokio::main]
async fn main() {
	let args = Args::parse();
	let base = args
		.server
		.parse::<reqwest::Url>()
		.expect("Should be valid uri");
	let mut config = api::apis::configuration::Configuration {
		base_path: args.server,
		..Default::default()
	};
	let server_version = api::apis::default_api::version_get(&config).await.unwrap();
	println!("Server: {}, version {:?}", base, server_version);
	let token = api::apis::default_api::login_get(&config, &args.password)
		.await
		.unwrap();
	println!("Login successful, token: {token}");
	config.api_key = Some(ApiKey {
		key: token,
		prefix: None,
	});
	while work_loop(&config).await {}
}
