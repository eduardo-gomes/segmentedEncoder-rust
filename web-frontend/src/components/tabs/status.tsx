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
		}).finally(() => setTimeout(doTrigger, 5000));
	})
	return status
}

function Job(props: { id: string, api: Accessor<DefaultApi> }) {
	const status = statusWatcher(props);
	return (<div>
		Job id: <samp>{props.id}</samp> output status: {status()}
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
		Auto refreshing /latest:
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