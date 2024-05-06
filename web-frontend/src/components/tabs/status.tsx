import { type Accessor, createEffect, createSignal, For, Match, onCleanup, Switch, useContext } from "solid-js";
import { ApiContext } from "../../lib/apiProvider";
import type { DefaultApi } from "@api";

function statusWatcher({ id, api }: { id: string, api: Accessor<DefaultApi> }): Accessor<string> {
	const [status, setStatus] = createSignal("pending");
	const [trigger, doTrigger] = createSignal(undefined, { equals: false });
	createEffect(() => {
		trigger();
		api().jobJobIdOutputGetRaw({ jobId: id }).then((response) => {
			if (response.raw.ok) setStatus("finished");
		}).catch(() => setTimeout(doTrigger, 5000));
	})
	return status
}

function Job(props: { id: string, api: Accessor<DefaultApi> }) {
	const status = statusWatcher(props);
	const ok = () => status() == "finished";
	const [progress, setProgress] = createSignal<number | undefined>();

	async function download() {
		setProgress(0);
		const response = await props.api().jobJobIdOutputGetRaw({ jobId: props.id }).then((r) => r.raw);
		if (!response.body) throw new Error("No body to download");
		const tracker = new TransformStream<Uint8Array, Uint8Array>({
			transform(chunk, controller) {
				setProgress((last) => (last ?? 0) + chunk.length);
				controller.enqueue(chunk);
			}
		});
		const blob = await (new Response(response.body.pipeThrough(tracker), { headers: { content_type: "video/matroska" } })).blob();
		const url = URL.createObjectURL(blob);
		const a = document.createElement<"a">('a')
		a.href = url
		a.download = props.id + ".mkv"
		document.body.appendChild(a)
		a.click()
		document.body.removeChild(a)
		URL.revokeObjectURL(url);
	}

	return (<div>
		Job id: <samp>{props.id}</samp> output status: {status()} {ok() ?
		<button onClick={download}>Download</button> : null} {progress()}
	</div>);
}

function StatusTab(props: { visible: boolean }) {
	const [status, setStatus] = createSignal<Array<string>>([]);

	const { authenticated, api } = useContext(ApiContext);

	const [trigger, doTrigger] = createSignal(undefined, { equals: false });
	createEffect(() => {
		trigger();
		if (!props.visible) return;
		const controller = new AbortController();
		onCleanup(() => {
			controller.abort("Cleanup")
		});
		api().jobGet({ signal: controller.signal }).then(setStatus).finally(() => setTimeout(doTrigger, 5000));
	});

	return (<>
		Auto refreshing job list:
		<Switch fallback={<div style={{ "font-size": "x-large" }}>Not connected to the server</div>}>
			<Match when={authenticated()}>
				<For each={status()}>
					{(element) => <Job id={element} api={api}/>}
				</For>
			</Match>
		</Switch>
	</>);
}

export default StatusTab;