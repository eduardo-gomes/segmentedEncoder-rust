use clap::Parser;

#[derive(Parser, Debug)]
#[command()]
struct Args {
	///Server api base url
	#[arg(short, long, default_value = "http://localhost:8888/api")]
	server: String,
}

fn main() {
	let args = Args::parse();
	let base = args
		.server
		.parse::<reqwest::Url>()
		.expect("Should be valid uri");
	println!("Server: {}", base);
	unimplemented!()
}
