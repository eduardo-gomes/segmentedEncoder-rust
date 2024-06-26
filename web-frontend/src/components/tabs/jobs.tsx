import "./jobs.css";
import { createSignal, Show, useContext } from "solid-js";
import type { Task } from "../../lib/task";
import { textChange } from "../../lib/utils";
import { ApiContext } from "../../lib/apiProvider";

function JobsTab() {
	const { api, authenticated } = useContext(ApiContext);
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

	function onCreate() {
		setStatus("Uploading job!");
		const task = get_task();
		api().jobPost({
			audioCodec: task.audio_encoder,
			audioParam: task.audio_args.split(" "),
			videoCodec: task.video_encoder,
			videoParam: task.video_args.split(" "),
			// eslint-disable-next-line @typescript-eslint/ban-ts-comment
			// @ts-ignore
			body: task.file,
			segmentDuration: 0,

		}).then((res) => {
			setStatus("Created job " + res);
		}).catch((e) => setStatus(e))
	}

	const [status, setStatus] = createSignal("");
	return (<>
			Add job:
			<div class="job-div">
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
				<button onClick={onCreate} disabled={!authenticated()}>
					Add job
					{!authenticated() ? " (missing authentication)" : undefined}
				</button>
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