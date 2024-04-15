import { ApiContextOld } from "../../lib/api_old";
import { createEffect, createSignal, Match, onCleanup, Switch, useContext } from "solid-js";

function StatusTab(props: { visible: boolean }) {
	const [status, setStatus] = createSignal("");

	const {path_on_url, is_connected} = useContext(ApiContextOld);

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

	let waiting_last = false;

	function status_updater() {
		if (waiting_last) return;//Ongoing request
		waiting_last = true;
		refresh().catch((e) => console.error("Failed to update status:", e)).finally(() => waiting_last = false);
	}

	createEffect(() => {
		const should_run = props.visible && is_connected();
		if (!should_run) return;
		const interval = setInterval(status_updater, 1000);
		onCleanup(() => {
			clearTimeout(interval);
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