// What the actor views call a request's choices, and how they sum up what
// the user said in response to one.

import type {
	Actor,
	ActorSession,
	ActorFileChangeKind,
	ActorPermissionDecision,
	ActorUserContent
} from '$lib/protocol/types';

/** Where `actor` works, by its last part ("cohesion" for a project's directory). */
export function scopeName(actor: Actor): string {
	return actor.scope.split(/[\\/]/).filter(Boolean).at(-1) ?? actor.scope;
}

/**
 * The model `session` runs on and its effort, as the model picker words them
 * (`Opus · High`); a session on the default runs on its actor's first model.
 * Null when neither says.
 */
export function modelLabel(session: ActorSession, actor: Actor | null): string | null {
	const identifier = session.model ?? actor?.models[0]?.identifier;
	if (identifier === undefined) return null;
	const known = actor?.models.find((m) => m.identifier === identifier);
	const parts = [known?.title ?? identifier];
	const effort = known?.efforts.find((e) => e.identifier === session.effort);
	if (effort) parts.push(effort.title);
	return parts.join(' · ');
}

/** A permission decision's button. */
export function decisionLabel(decision: ActorPermissionDecision): string {
	switch (decision) {
		case 'allow':
			return 'Allow';
		case 'allowForSession':
			return 'Allow for this session';
		case 'deny':
			return 'Deny';
		case 'abort':
			return 'Deny and stop';
	}
}

export function fileChangeLabel(change: ActorFileChangeKind): string {
	switch (change) {
		case 'add':
			return 'Add';
		case 'update':
			return 'Edit';
		case 'delete':
			return 'Delete';
	}
}

/** What the user said, as their bubble shows it. */
export function userText(content: ActorUserContent): string {
	switch (content.kind) {
		case 'text':
			return content.text;
		case 'answers':
			return content.answers
				.map((answer) => [...answer.selected, answer.other].filter(Boolean).join(', '))
				.join('\n');
		case 'permission': {
			const decided = {
				allow: 'Allowed',
				allowForSession: 'Allowed for this session',
				deny: 'Denied',
				abort: 'Denied and asked the actor to stop'
			}[content.decision];
			return content.message ? `${decided}: ${content.message}` : `${decided}.`;
		}
		case 'plan':
			if (content.approved) return 'Approved the plan.';
			return content.feedback ? `Asked for changes: ${content.feedback}` : 'Asked for changes.';
	}
}
