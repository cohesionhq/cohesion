import { describe, expect, it } from 'vitest';
import type { AgentRunPart } from '$lib/protocol/types';
import type { AgentMessage } from '$lib/stores/agents.svelte';
import { collapseRuns } from './runs';

function message(id: number, run_id: number | null, run_part: AgentRunPart | null): AgentMessage {
	return { id, run_id, run_part } as AgentMessage;
}

const ids = (messages: AgentMessage[]) => messages.map((m) => m.id);

describe('collapseRuns', () => {
	it('keeps a fork’s trigger and what it surfaces, with its steps under the trigger', () => {
		const { shown, steps } = collapseRuns([
			message(1, null, null),
			message(2, 7, 'trigger'),
			message(3, 7, 'step'),
			message(4, 7, 'surfaced'),
			message(5, 7, 'step')
		]);
		expect(ids(shown)).toEqual([1, 2, 4]);
		expect(ids(steps.get(7) ?? [])).toEqual([3, 5]);
	});

	it('goes by run when forks and the conversation interleave', () => {
		const { shown, steps } = collapseRuns([
			message(1, 7, 'trigger'),
			message(2, 8, 'trigger'),
			message(3, 7, 'step'),
			message(4, null, null),
			message(5, 8, 'step'),
			message(6, 7, 'step')
		]);
		expect(ids(shown)).toEqual([1, 2, 4]);
		expect(ids(steps.get(7) ?? [])).toEqual([3, 6]);
		expect(ids(steps.get(8) ?? [])).toEqual([5]);
	});

	it('lets the first step stand for a fork whose trigger isn’t loaded', () => {
		const { shown, steps, triggers } = collapseRuns([
			message(3, 7, 'step'),
			message(4, 7, 'surfaced'),
			message(5, 7, 'step')
		]);
		expect(ids(shown)).toEqual([3, 4]);
		expect(ids(steps.get(7) ?? [])).toEqual([3, 5]);
		expect(triggers.has(7)).toBe(false);
	});
});
