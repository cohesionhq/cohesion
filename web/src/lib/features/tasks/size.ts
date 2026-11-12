import type { TaskSize } from '$lib/protocol/types';
import { vocabulary } from '$lib/protocol/vocabulary.svelte';
import { agentChip, agentText } from '$lib/ui/colors';

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
		chip: agentChip[color],
		tone: agentText[color],
		key: String(i + 1)
	}));
}

/** One size's label and the classes that color its chip and text. */
export function sizeOf(value: TaskSize) {
	return sizes().find((s) => s.value === value)!;
}
