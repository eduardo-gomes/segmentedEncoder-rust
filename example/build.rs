fn main() -> Result<(), Box<dyn std::error::Error>> {
	tonic_build::compile_protos("proto/status.proto")?;
	let path = std::path::PathBuf::from("web-root");
	web_packer::pack_web(&path)?;
	Ok(())
}
