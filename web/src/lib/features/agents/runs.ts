// Gathers a fork's steps under what woke it, by the `run_id` and `run_part` the
// daemon gives each message. A fork's messages may fall among others, so they
// go by run rather than by position.

import type { AgentMessage } from '$lib/stores/agents.svelte';

export interface CollapsedRuns {
	/**
	 * What the main transcript shows, in order: everything but steps, and for a
	 * fork whose trigger isn't loaded (it is on an older page, or the fork is
	 * from before wake lines) its first step, which stands for it.
	 */
	shown: AgentMessage[];
	/** Each fork's steps, in order, by run. */
	steps: Map<number, AgentMessage[]>;
	/** The loaded trigger of each fork, by run. */
	triggers: Map<number, AgentMessage>;
}

/** Gathers `messages`, oldest first, by the fork each belongs to. */
export function collapseRuns(messages: AgentMessage[]): CollapsedRuns {
	const steps = new Map<number, AgentMessage[]>();
	const triggers = new Map<number, AgentMessage>();
	for (const message of messages) {
		if (message.run_id === null) continue;
		if (message.run_part === 'trigger') triggers.set(message.run_id, message);
		if (message.run_part !== 'step') continue;
		const run = steps.get(message.run_id);
		if (run) run.push(message);
		else steps.set(message.run_id, [message]);
	}
	const shown = messages.filter(
		(message) =>
			message.run_part !== 'step' ||
			message.run_id === null ||
			(!triggers.has(message.run_id) && steps.get(message.run_id)?.[0] === message)
	);
	return { shown, steps, triggers };
}
