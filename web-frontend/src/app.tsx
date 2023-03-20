import "./style.css"
import { createSignal } from "solid-js";
import { ApiProvider } from "./lib/api";
import { hashIntegration, Router } from "@solidjs/router";
import { TabBar } from "./components/tabs";
import StatusTab from "./components/tabs/status";
import JobsTab from "./components/tabs/jobs";
import ServerSelector from "./components/tabs/serverSelector";

export function App() {
	const [statusActive, setStatusActive] = createSignal(false);
	return (
		<Router source={hashIntegration()}>
			<ApiProvider>
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
			</ApiProvider>
		</Router>
	)
}