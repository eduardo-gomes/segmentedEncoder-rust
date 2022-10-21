import Tab from "./tab";

const status_div = document.createElement("div");
const out = document.createElement("pre");
status_div.insertAdjacentHTML("afterbegin", "Auto refreshing /latest:");
status_div.appendChild(out);

async function refresh() {
	let res;
	try {
		res = await fetch("/api/status");
	} catch (e) {
		const message = "Fetch failed";
		out.innerText = message;
		throw new Error(message, {cause: e as Error});
	}
	if (res.status >= 400) {
		const message = `Refresh got status code: ${res.status}`;
		out.innerText = message;
		throw new Error(message);
	}
	out.innerText = await res.text();
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

const status_tab = new Tab(status_div, "Status", foreground, background);
export default status_tab;