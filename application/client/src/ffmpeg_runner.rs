use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::future::Future;
use std::process::{ExitStatus, Stdio};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{ChildStdout, Command};
use tokio::sync::mpsc::{channel, Receiver};

struct Status(pub BTreeMap<String, String>);

fn status_adapter(stream: impl AsyncRead + Unpin + Send + 'static) -> Receiver<Status> {
	let mut stream = BufReader::new(stream);
	let (sender, receiver) = channel(32);
	tokio::spawn(async move {
		let mut status = BTreeMap::new();
		loop {
			let mut line = String::new();
			if stream.read_line(&mut line).await.is_err() {
				break;
			}
			if let Some((name, value)) = line.split_once('=') {
				status.insert(name.into(), value.trim_end().into());
			}
			let is_complete = line.starts_with("progress=");
			if is_complete {
				if sender.send(Status(status)).await.is_err() {
					break;
				}
				status = BTreeMap::new();
			}
		}
	});
	receiver
}

pub(crate) fn run_to_stream<I, S>(
	args: I,
) -> (
	ChildStdout,
	impl Future<Output = std::io::Result<ExitStatus>>,
)
where
	I: IntoIterator<Item = S>,
	S: AsRef<OsStr>,
{
	let mut ffmpeg = Command::new("ffmpeg");
	ffmpeg.args(args);
	ffmpeg.args(["-progress", "pipe:2", "-nostats", "-v", "quiet"]);
	ffmpeg.args(["-f", "matroska", "-"]);
	ffmpeg
		.stderr(Stdio::piped())
		.stdout(Stdio::piped())
		.stdin(Stdio::null());
	println!("ffmpeg command: {:?}", ffmpeg);
	let mut child = ffmpeg.spawn().unwrap();
	let output = child.stdout.take().unwrap();
	let progress = child.stderr.take().unwrap();
	let status = async move { child.wait().await };
	let parsed_progress = status_adapter(progress);
	tokio::spawn(async move {
		let mut stream = parsed_progress;
		loop {
			let status = match stream.recv().await {
				None => return,
				Some(status) => status,
			};
			status
				.0
				.iter()
				.filter(|(key, val)| {
					key.as_str().eq("out_time")
						|| (key.as_str(), val.as_str()) == ("progress", "end")
				})
				.for_each(|(_, val)| println!("Time: {val}"));
		}
	});
	(output, status)
}
