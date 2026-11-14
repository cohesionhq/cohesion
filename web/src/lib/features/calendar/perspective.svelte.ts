// Bypass perspective: whether the calendar page shows every calendar in
// Cohesion rather than the ones this workspace chose. A choice for this visit
// only, per workspace: it resets when the page reloads.

import { calendars } from '$lib/stores/calendars.svelte';

class PerspectiveBypass {
	#workspaces = $state<number[]>([]);

	isOn(workspace_id: number): boolean {
		return this.#workspaces.includes(workspace_id);
	}

	set(workspace_id: number, on: boolean): void {
		const others = this.#workspaces.filter((id) => id !== workspace_id);
		this.#workspaces = on ? [...others, workspace_id] : others;
	}
}

export const perspectiveBypass = new PerspectiveBypass();

/** Whether workspace `workspace_id`'s calendar page draws calendar `calendar_id`. */
export function isDrawn(workspace_id: number, calendar_id: number): boolean {
	return perspectiveBypass.isOn(workspace_id) || calendars.inPerspective(calendar_id);
}
