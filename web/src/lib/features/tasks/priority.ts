import type { TaskPriority } from '$lib/protocol/types';
import { vocabulary } from '$lib/protocol/vocabulary.svelte';

/** The colour of each priority's flag: urgent, high and medium take a hue. */
const TONES: Record<TaskPriority, string> = {
	urgent: 'text-danger',
	high: 'text-agent-orange',
	medium: 'text-accent',
	low: 'text-fg-subtle',
	none: 'text-fg-subtle'
};

/** On a focused row, 1–4 set the priorities most urgent first and 0 clears it. */
export const PRIORITY_KEYS: Record<string, TaskPriority> = {
	'1': 'urgent',
	'2': 'high',
	'3': 'medium',
	'4': 'low',
	'0': 'none'
};

/** Each priority, most urgent first, with its label, the colour of its flag and its key in a menu. */
export function priorities(): { value: TaskPriority; label: string; tone: string; key: string }[] {
	return vocabulary.value.task_priorities.map(({ kind, label }) => ({
		value: kind,
		label,
		tone: TONES[kind],
		key: Object.keys(PRIORITY_KEYS).find((k) => PRIORITY_KEYS[k] === kind)!
	}));
}

export function priorityOf(value: TaskPriority) {
	return { value, label: vocabulary.priority(value), tone: TONES[value] };
}
