import { createSignal, Setter, Show, useContext } from "solid-js";
import type { Task } from "./lib/task";
import { create_task } from "./lib/task";
import { ApiContext } from "./lib/api";

function JobsTab() {
	const api = useContext(ApiContext);
	const [videoCodec, setVideoCodec] = createSignal("libsvtav1");
	const [videoArgs, setVideoArgs] = createSignal("-preset 4 -crf 27");
	const [audioCodec, setAudioCodec] = createSignal("libopus");
	const [audioArgs, setAudioArgs] = createSignal("-b:a 96k");

	let file_input: HTMLInputElement | undefined;

	function get_task() {
		const files = file_input?.files;
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

	function onCreate() {
		setStatus("Uploading job!");
		create_task(api, get_task()).then((res) => {
				if (!res.isErr)
					setStatus("Created job " + res.job);
				else
					setStatus(res.text);
			}
		)
	}

	const [status, setStatus] = createSignal("");
	return (<>
			Add job:
			<div id="job-div">
				<label>
					<span>Input file:</span>
					<input ref={ref => file_input = ref} type="file" accept="video/*"/>
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
				<input type="button" value="Add job" onClick={onCreate}/>
				<Show when={status()}>
					<label>
						<span>Status:</span>
						<span>{status()}</span>
					</label>
				</Show>
			</div>
		</>
	);
}

export default JobsTab;