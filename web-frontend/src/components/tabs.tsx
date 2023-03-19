import { A, Navigate, Route, Routes, useMatch } from '@solidjs/router';
import type { JSX } from 'solid-js';
import { createEffect, createMemo, For, mapArray } from "solid-js";

type TabElement = {
	title: string,
	component: JSX.Element,
	visibilityChange?: (visible: boolean) => void
};

function TabBar(props: { children: TabElement[] }) {
	const c = createMemo(() => props.children);

	//Track visibility and notify callback
	mapArray(c, (el) => {
		const match = useMatch(() => el.title);
		createEffect(() => {
			const visibility = Boolean(match());
			el.visibilityChange?.(visibility);
		});
	})();

	return (
		<>
			<div class={"tabs"}>
				<For each={c()}>
					{(tab) =>
						<A href={tab.title}>
							<button>{tab.title}</button>
						</A>}
				</For>
			</div>
			<Routes>
				<Route path={"/*"} element={<Navigate href={"status"}/>}/>
				<For each={c()}>
					{(tab) =>
						<Route path={tab.title} element={tab.component}/>}
				</For>
			</Routes>
		</>
	)
}

export { TabBar };