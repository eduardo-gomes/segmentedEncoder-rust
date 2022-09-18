import status_div, {status_updater} from "./status";
import jobs_div from "./jobs";

const tabs = document.getElementById("tabs") as HTMLDivElement;
const container = document.getElementById("container") as HTMLDivElement;
const div_list = new Map<string, HTMLDivElement>();

function show_div(div: HTMLDivElement) {
	div_list.forEach((div) => div.classList.add("disabled"));
	div.classList.remove("disabled");
}

function add_tab(element: HTMLDivElement, label: string) {
	div_list.set(label, element);
	const button = document.createElement("button");
	button.innerText = label;
	tabs.appendChild(button);
	element.classList.add("disabled");
	container.appendChild(element);

	button.addEventListener("click", () => show_div(element));
}

add_tab(status_div, "Status");
add_tab(jobs_div, "Jobs");
show_div(status_div);
status_updater(2000);

console.log("Js file loaded");