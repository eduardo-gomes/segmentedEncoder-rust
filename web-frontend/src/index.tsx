import "./style.css"
import StatusTab from "./components/tabs/status";
import JobsTab from "./components/tabs/jobs";
import ServerSelector from "./components/tabs/serverSelector";
import { TabBar } from "./components/tabs";
import { render } from "solid-js/web";
import { createSignal } from "solid-js";
import { ApiProvider } from "./lib/api";

import { hashIntegration, Router } from "@solidjs/router";

function App() {
	const [statusActive, setStatusActive] = createSignal(false);
	return (
		<Router source={hashIntegration()}>
			<TabBar>
				{[
					{
						title: "Status",
						component: <StatusTab visible={statusActive()}/>,
						visibilityChange: setStatusActive
					},
					{
						title: "Jobs",
						component: <JobsTab/>,
					},
					{
						title: "Server",
						component: <ServerSelector/>,
					},
				]}
			</TabBar>
		</Router>
	)
}

render(() => (
	<ApiProvider url={new URL("http://localhost:8888/api")}>
		<App/>
	</ApiProvider>
), document.getElementById("App") as HTMLDivElement);