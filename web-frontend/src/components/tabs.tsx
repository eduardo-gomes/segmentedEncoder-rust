import { A, Route, Routes, useMatch } from '@solidjs/router';
import type { JSX } from 'solid-js';
import { createEffect, createMemo, For, mapArray } from "solid-js";

type TabElement = {
	title: string,
	component: JSX.Element,
	visibilityChange?: (visible: boolean) => void
};

type NonEmptyArray<T> = [T, ...T[]];

function TabBar(props: { children: NonEmptyArray<TabElement> }) {
	const c = createMemo(() => props.children);

	const match_root = useMatch(() => "");
	//Track visibility and notify callback
	mapArray(c, (el, i) => {
		const match = useMatch(() => el.title);
		createEffect(() => {
			const root_vis = i() === 0 ? Boolean(match_root()) : false;
			const visibility = Boolean(match()) || root_vis;
			el.visibilityChange?.(visibility);
		});
	})();

	return (
		<>
			<div class="tabs">
				<For each={c()}>
					{(tab) =>
						<A href={tab.title}>
							<button>{tab.title}</button>
						</A>}
				</For>
			</div>
			<Routes>
				<Route path="/" element={c()[0].component}/>
				<For each={c()}>
					{(tab) =>
						<Route path={tab.title} element={tab.component}/>}
				</For>
			</Routes>
		</>
	)
}

export { TabBar };