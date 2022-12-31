import { get_api_path } from "./lib/api";
import { createSignal, Setter, Show } from "solid-js";

function create_task(task: Task, setStatus: (status: string) => void) {
	function send_fulfilled(res: Response) {
		console.debug("Created task response:", res);
		if (res.ok)
			res.text().then(function (text) {
				const url = new URL(`/api/jobs/${text}/source`, window.location.origin);
				console.info("Source available at:", url.href)
				setStatus("Created job " + text);
			});
		else {
			console.warn("Request was not successful:", res);
			setStatus("Created job bad response");
		}
	}

	function send_rejected(e: unknown) {
		console.error("Create task error:", e);
		setStatus("Upload job request failed");
	}

	console.debug("Task to create:", task);

	setStatus("Uploading job!");

	send_task(task)
		.then(send_fulfilled)
		.catch(send_rejected);
}

type Task = {
	video_encoder: string,
	video_args: string,
	audio_encoder: string,
	audio_args: string,
	file: File
};

function visible_ascii(str: string) {
	//Only ascii will be allowed for now.
	//For previous encoding: https://github.com/eduardo-gomes/segmentedEncoder-rust/blob/b82d0ea872d5784cedbac3003db6df6e09ccbf37/web-frontend/web-root/jobs.ts#L77
	const match = str.match(/[^\x20-\x7E\t]|%/g);
	if (match) throw Error("Found character not in visible ascii: " + match.join());
	return str;
}

async function send_task(task: Task) {
	const headers = {
		video_encoder: visible_ascii(task.video_encoder),
		video_args: visible_ascii(task.video_args),
		audio_encoder: visible_ascii(task.audio_encoder),
		audio_args: visible_ascii(task.audio_args),
	};
	console.debug("Encoded header:", headers);
	return await fetch(get_api_path() + "/jobs", {
		method: "POST",
		headers: headers,
		body: task.file
	})
}

function JobsTab() {
	const [videoCodec, setVideoCodec] = createSignal("libsvtav1");
	const [videoArgs, setVideoArgs] = createSignal("-preset 4 -crf 27");
	const [audioCodec, setAudioCodec] = createSignal("libopus");
	const [audioArgs, setAudioArgs] = createSignal("-b:a 96k");

	let file_list: FileList | null;

	function update_files(event: Event) {
		const file_input = event.target as HTMLInputElement;
		file_list = file_input.files;
	}

	function get_task() {
		const files = file_list;
		if (files == null || files.length < 1) throw new Error("No file selected");
		const task: Task = {
			video_encoder: videoCodec(),
			video_args: videoArgs(),
			audio_encoder: audioCodec(),
			audio_args: audioArgs(),
			file: files[0]
		};
		return task;
	}

	function textChange(fn: Setter<string>) {
		return (e: Event & { currentTarget: HTMLInputElement }) => fn(e.currentTarget.value);
	}

	const [status, setStatus] = createSignal("");
	return (<>
			Add job:
			<div id="job-div">
				<label>
					<span>Input file:</span>
					<input type="file" accept="video/*" onChange={update_files}/>
				</label>
				<label>
					<span>video encoder:</span>
					<input type="text" value={videoCodec()} onChange={textChange(setVideoCodec)}/></label>
				<label>
					<span>video args:</span>
					<input type="text" value={videoArgs()} onChange={textChange(setVideoArgs)}/></label>
				<label>
					<span>audio encoder:</span>
					<input type="text" value={audioCodec()} onChange={textChange(setAudioCodec)}/></label>
				<label>
					<span>audio args:</span>
					<input type="text" value={audioArgs()} onChange={textChange(setAudioArgs)}/>
				</label>
				<input type="button" value="Add job" onClick={() => create_task(get_task(), setStatus)}/>
				<label class={status() ? undefined : "disabled"}>
					<span>Status:</span>
					<span>{status()}</span>
				</label>
			</div>
		</>
	);
}

export default JobsTab;