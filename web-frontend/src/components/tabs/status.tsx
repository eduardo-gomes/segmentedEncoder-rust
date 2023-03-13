import { ApiContext } from "../../lib/api";
import { createEffect, createSignal, Match, onCleanup, Switch, useContext } from "solid-js";

function StatusTab(props: { visible: boolean }) {
	const [status, setStatus] = createSignal("");

	const {path_on_url, is_connected} = useContext(ApiContext);

	async function refresh() {
		let res;
		try {
			res = await fetch(path_on_url("/status"));
		} catch (e) {
			const message = "Fetch failed";
			setStatus(message);
			console.warn(Error(message, {cause: e as Error}));
			return;
		}
		if (res.status >= 400) {
			const message = `Refresh got status code: ${res.status}`;
			setStatus(message);
			throw new Error(message);
		}
		setStatus(await res.text());
		console.debug("Request got:", res.status);
	}

	let timeout: undefined | number;
	let should_rerun = false;

	function rerun() {
		clearTimeout(timeout);
		if (should_rerun)
			timeout = setTimeout(status_updater, 1000);
	}

	function status_updater() {
		refresh().catch((e) => console.error("Failed to update status:", e)).finally(rerun);
	}

	createEffect(() => {
		should_rerun = props.visible && is_connected();
		if (should_rerun)
			status_updater();
		onCleanup(() => {
			should_rerun = false;
			clearTimeout(timeout);
		});
	});

	return (<>
		Auto refreshing /latest:
		<Switch fallback={<div style={{"font-size": "x-large"}}>Not connected to the server</div>}>
			<Match when={is_connected()}>
				<pre>{status()}</pre>
			</Match>
		</Switch>
	</>);
}

export default StatusTab;