import status_div from "./status";
import jobs_div from "./jobs";
import type Tab from "./tab";

const tabs = document.getElementById("tabs") as HTMLDivElement;
const container = document.getElementById("container") as HTMLDivElement;
const tab_list = new Map<string, Tab>();

function show_tab(tab: Tab) {
	tab_list.forEach((tab) => tab.hide());
	tab.show();
}

function add_tab(tab: Tab) {
	tab_list.set(tab.label, tab);
	const button = document.createElement("button");
	button.innerText = tab.label;
	tabs.appendChild(button);
	tab.hide();
	container.appendChild(tab.element);

	button.addEventListener("click", () => show_tab(tab));
}

add_tab(status_div);
add_tab(jobs_div);
show_tab(status_div);

console.log("Js file loaded");