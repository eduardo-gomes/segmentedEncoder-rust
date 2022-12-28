import {get_api_path} from "./lib/api";
import {createEffect, createSignal, onCleanup} from "solid-js";

function StatusTab(props: { visible: boolean }) {
	const [status, setStatus] = createSignal("");

	async function refresh() {
		let res;
		try {
			res = await fetch(get_api_path() + "/status");
		} catch (e) {
			const message = "Fetch failed";
			setStatus(message);
			throw new Error(message, {cause: e as Error});
		}
		if (res.status >= 400) {
			const message = `Refresh got status code: ${res.status}`;
			setStatus(message);
			throw new Error(message);
		}
		setStatus(await res.text());
		console.debug("Request got:", res.status);
	}

	function status_updater() {
		refresh().then().catch((e) => console.error("Failed to update status:", e));
	}

	let interval: undefined | number;

	function foreground() {
		if (interval != undefined) return;
		interval = setInterval(status_updater, 2000);
		status_updater();
	}

	function background() {
		if (interval === undefined) return;
		clearInterval(interval);
		interval = undefined;
	}

	createEffect(() => props.visible ? foreground() : background());
	onCleanup(() => clearInterval(interval));

	return (<>
		Auto refreshing /latest:
		<pre>{status()}</pre>
	</>);
}

export default StatusTab;