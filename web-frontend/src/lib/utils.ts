import type { Accessor, Setter } from "solid-js";
import { createSignal } from "solid-js";

/**
 * Generate callback to update text for change events on input elements
 */
function textChange(fn: Setter<string>) {
	return (e: Event & { currentTarget: HTMLInputElement }) => fn(e.currentTarget.value);
}

export { textChange };

export interface Signal<T> {
	get: Accessor<T>,
	set: Setter<T>,
}

export function createSignalObj<T>(val: T): Signal<T> {
	const [get, set] = createSignal(val);
	return { get, set }
}