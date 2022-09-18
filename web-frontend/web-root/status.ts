const status_div = document.createElement("div");
const out = document.createElement("pre");
status_div.insertAdjacentHTML("afterbegin", "Auto refreshing /latest:");
status_div.appendChild(out);

async function refresh() {
	let res = await fetch("/latest");
	if (res.status >= 400) {
		const message = `Refresh got status code: ${res.status}`;
		out.innerText = message;
		throw new Error(message);
	}
	out.innerText = await res.text();
	return "Request got: " + res.status;
}

function status_updater(timeout: number) {
	function handler() {
		refresh().then(console.debug).catch(console.error)
	}

	setInterval(handler, timeout);
}

export {status_updater};
export default status_div;