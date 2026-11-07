import type { IntelligenceAgentColor, TaskSize } from '$lib/protocol/types';
import { vocabulary } from '$lib/protocol/vocabulary.svelte';

/** A size's chip: its color as text on a faint wash of the same color. */
const CHIPS: Record<IntelligenceAgentColor, string> = {
	blue: 'bg-agent-blue/15 text-agent-blue',
	green: 'bg-agent-green/15 text-agent-green',
	orange: 'bg-agent-orange/15 text-agent-orange',
	purple: 'bg-agent-purple/15 text-agent-purple',
	red: 'bg-agent-red/15 text-agent-red',
	pink: 'bg-agent-pink/15 text-agent-pink',
	teal: 'bg-agent-teal/15 text-agent-teal',
	yellow: 'bg-agent-yellow/15 text-agent-yellow'
};

/** A size's color as text alone, for a picker button. */
const TONES: Record<IntelligenceAgentColor, string> = {
	blue: 'text-agent-blue',
	green: 'text-agent-green',
	orange: 'text-agent-orange',
	purple: 'text-agent-purple',
	red: 'text-agent-red',
	pink: 'text-agent-pink',
	teal: 'text-agent-teal',
	yellow: 'text-agent-yellow'
};

/** The key that clears a size in a menu; the sizes take 1 for the smallest on up. */
export const NO_SIZE_KEY = '0';

/** Each size, smallest first, with its label, the classes that color its chip and its key in a menu. */
export function sizes(): {
	value: TaskSize;
	label: string;
	chip: string;
	tone: string;
	key: string;
}[] {
	return vocabulary.value.task_sizes.map(({ kind, label, color }, i) => ({
		value: kind,
		label,
		chip: CHIPS[color],
		tone: TONES[color],
		key: String(i + 1)
	}));
}

/** One size's label and the classes that color its chip and text. */
export function sizeOf(value: TaskSize) {
	return sizes().find((s) => s.value === value)!;
}
