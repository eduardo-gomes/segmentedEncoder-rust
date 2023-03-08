import type { Setter } from "solid-js";

/**
 * Generate callback to update text for change events on input elements
 */
function textChange(fn: Setter<string>) {
	return (e: Event & { currentTarget: HTMLInputElement }) => fn(e.currentTarget.value);
}

export { textChange };