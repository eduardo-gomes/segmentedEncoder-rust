import { get_path_on_api } from "./api";

type Successful = { job: string, isErr: false };
type Rejected = { text: string, isErr: true };

export function create_task(task: Task): Promise<Rejected | Successful> {
	async function send_fulfilled(res: Response): Promise<Rejected | Successful> {
		console.debug("Created task response:", res);
		if (res.ok) {
			const text = await res.text();
			const url = get_path_on_api(`/jobs/${text}/source`);
			console.info("Source available at:", url.href);
			return {job: text, isErr: false};
		} else {
			console.warn("Request was not successful:", res);
			return {text: "Created job bad response", isErr: true};
		}
	}

	function send_rejected(e: unknown): Rejected {
		console.error("Create task error:", e);
		return {text: "Upload job request failed", isErr: true};
	}

	console.debug("Task to create:", task);

	return send_task(task)
		.then(send_fulfilled)
		.catch(send_rejected);
}

export type Task = {
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
	return await fetch(get_path_on_api("/jobs"), {
		method: "POST",
		headers: headers,
		body: task.file
	})
}