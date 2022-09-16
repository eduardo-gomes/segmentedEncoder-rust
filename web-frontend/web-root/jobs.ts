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
input_div.appendChild(add_button);

export default jobs_div;