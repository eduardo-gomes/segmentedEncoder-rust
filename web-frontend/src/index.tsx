import "./style.css"
import status_div from "./status";
import jobs_div from "./jobs";
import {Tab, TabBar} from "./tabs";
import {render} from "solid-js/web";

//We need to make tab able to interrupt the work with reactivity
status_div.show();

function App() {
	return (
		<>
			<TabBar>
				<Tab title={"Status"}>
					{status_div.element}
				</Tab>
				<Tab title={"Jobs"}>
					{jobs_div.element}
				</Tab>
			</TabBar>
		</>
	)
}

render(() => <App/>, document.getElementById("App") as HTMLDivElement);