fn main() -> Result<(), Box<dyn std::error::Error>> {
	let path = std::path::PathBuf::from("web-root");
	web_packer::pack_web(&path)?;
	Ok(())
}
