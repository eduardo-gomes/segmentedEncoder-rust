const jobs_div = document.createElement("div");
const input_div = document.createElement("div");
jobs_div.appendChild(document.createTextNode("Add job:"));
jobs_div.appendChild(input_div);

input_div.style.display = "flex";
input_div.style.flexDirection = "column";

const file_input_label = document.createElement("label");
file_input_label.innerText = "Input file:"
const file_input = document.createElement("input");
file_input.type = "file";
file_input.accept = "video/*";
file_input_label.appendChild(file_input)
input_div.appendChild(file_input_label);

const video_codec_label = document.createElement("label");
video_codec_label.innerText = "video encoder:";
const video_codec = document.createElement("input");
video_codec.type = "text";
video_codec_label.appendChild(video_codec);
input_div.appendChild(video_codec_label);

const video_args_label = document.createElement("label");
video_args_label.innerText = "video args:";
const video_args = document.createElement("input");
video_args.type = "text";
video_args_label.appendChild(video_args);
input_div.appendChild(video_args_label);

const audio_codec_label = document.createElement("label");
audio_codec_label.innerText = "audio encoder:";
const audio_codec = document.createElement("input");
audio_codec.type = "text";
audio_codec_label.appendChild(audio_codec);
input_div.appendChild(audio_codec_label);

const audio_args_label = document.createElement("label");
audio_args_label.innerText = "audio args:";
const audio_args = document.createElement("input");
audio_args.type = "text";
audio_args_label.appendChild(audio_args);
input_div.appendChild(audio_args_label);

const add_button = document.createElement("input");
add_button.type = "button";
add_button.value = "Add job";
add_button.addEventListener("click", create_task);
input_div.appendChild(add_button);

function create_task() {
	let files = file_input.files;
	if (files == null || files.length < 1) throw new Error("No file selected");
	let task: Task = {
		video_encoder: video_codec.value,
		video_args: video_args.value,
		audio_encoder: audio_codec.value,
		audio_args: audio_args.value,
		file: files[0]
	};
	console.log("Task to create:", task);
}

type Task = {
	video_encoder: string,
	video_args: string,
	audio_encoder: string,
	audio_args: string,
	file: File
};

export default jobs_div;