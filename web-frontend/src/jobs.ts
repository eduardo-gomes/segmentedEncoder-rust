import Tab from "./tab";
import {get_api_path} from "./api";

const jobs_div = document.createElement("div");
const input_div = document.createElement("div");
input_div.id = "job-div";
jobs_div.appendChild(document.createTextNode("Add job:"));
jobs_div.appendChild(input_div);

function createLabel(text: string): HTMLLabelElement {
	const label = document.createElement("label");
	const span = document.createElement("span");
	span.innerText = text;
	label.appendChild(span);
	return label;
}

const file_input_label = createLabel("Input file:");
const file_input = document.createElement("input");
file_input.type = "file";
file_input.accept = "video/*";
file_input_label.appendChild(file_input)
input_div.appendChild(file_input_label);

const video_codec_label = createLabel("video encoder:");
const video_codec = document.createElement("input");
video_codec.type = "text";
video_codec_label.appendChild(video_codec);
input_div.appendChild(video_codec_label);

const video_args_label = createLabel("video args:");
const video_args = document.createElement("input");
video_args.type = "text";
video_args_label.appendChild(video_args);
input_div.appendChild(video_args_label);

const audio_codec_label = createLabel("audio encoder:");
const audio_codec = document.createElement("input");
audio_codec.type = "text";
audio_codec_label.appendChild(audio_codec);
input_div.appendChild(audio_codec_label);

const audio_args_label = createLabel("audio args:");
const audio_args = document.createElement("input");
audio_args.type = "text";
audio_args_label.appendChild(audio_args);
input_div.appendChild(audio_args_label);

const add_button = document.createElement("input");
add_button.type = "button";
add_button.value = "Add job";
add_button.addEventListener("click", create_task);
input_div.appendChild(add_button);

const status_label = createLabel("Status:");
const status_text = document.createElement("span");
status_label.classList.add("disabled");
status_label.appendChild(status_text)
input_div.appendChild(status_label);

function create_task() {
	function get_input() {
		let files = file_input.files;
		if (files == null || files.length < 1) throw new Error("No file selected");
		let task: Task = {
			video_encoder: video_codec.value,
			video_args: video_args.value,
			audio_encoder: audio_codec.value,
			audio_args: audio_args.value,
			file: files[0]
		};
		return task;
	}

	function send_fulfilled(res: Response) {
		console.debug("Created task response:", res);
		if (res.ok)
			res.text().then(function (text) {
				let url = new URL(`/api/jobs/${text}/source`, window.location.origin);
				console.info("Source available at:", url.href)
				status_text.innerText = "Created job " + text;
			});
		else {
			console.warn("Request was not successful:", res);
			status_text.innerText = "Created job bad response";
		}
	}

	function send_rejected(e: any) {
		console.error("Create task error:", e);
		status_text.innerText = "Upload job request failed";
	}

	let task = get_input();
	console.debug("Task to create:", task);

	status_label.classList.remove("disabled");
	status_text.innerText = "Uploading job!";

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

function fill_default_values() {
	video_codec.value = "libsvtav1";
	video_args.value = "-preset 4 -crf 27";
	audio_codec.value = "opus";
	audio_args.value = "-b:a 96k";
}

fill_default_values();

const jobs_tab = new Tab(jobs_div, "Jobs");
export default jobs_tab;