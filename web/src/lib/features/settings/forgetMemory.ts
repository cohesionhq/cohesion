// What forgetting a memory says, wherever a memory is forgotten from: who
// will no longer see it.

/** Who sees a memory: every agent, a team's agents or one agent. */
export type Seers =
	{ kind: 'global' } | { kind: 'team'; title: string } | { kind: 'agent'; name: string };

/** The confirmation's words for forgetting a memory `seers` see. */
export const noLongerSeen = (seers: Seers) => {
	switch (seers.kind) {
		case 'global':
			return 'No agent will see this memory again. This can’t be undone.';
		case 'team':
			return `Agents on ${seers.title} will no longer see this memory. This can’t be undone.`;
		case 'agent':
			return `${seers.name} will no longer see this memory. This can’t be undone.`;
	}
};
