import status_div, {status_updater} from "./status";
import jobs_div from "./jobs";

const tabs = document.getElementById("tabs");
const container = document.getElementById("container");
const div_list = new Map<string, HTMLDivElement>();

function add_tab(element: HTMLDivElement, label: string) {
	div_list.set(label, element);
	const button = document.createElement("button");
	button.innerText = label;
	tabs.appendChild(button);
	element.classList.add("disabled");
	container.appendChild(element);

	let callback = () => {
		div_list.forEach((div) => div.classList.add("disabled"));
		element.classList.remove("disabled");
	};
	button.addEventListener("click", callback)
}

add_tab(status_div, "Status");
add_tab(jobs_div, "Jobs");
status_updater(500);

console.log("Js file loaded");