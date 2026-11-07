// How the actor views show a share of a limit used: a session's context
// window, or one of its actor's plan limits.

import { shortTime } from '$lib/format/dates';
import type { ListedActorUsage, PlanUsage } from '$lib/protocol/types';

export type UsageTone = 'accent' | 'warn' | 'danger';

/** The color for a share used: the accent, then warning past 80%, then danger past 95%. */
export function usageTone(share: number): UsageTone {
	if (share >= 0.95) return 'danger';
	if (share >= 0.8) return 'warn';
	return 'accent';
}

/** "84%". */
export function usagePercent(share: number): string {
	return `${Math.round(share * 100)}%`;
}

/** "Resets at 15:10" within a day of `now`, else "Resets Mon 15:10"; both in Unix seconds. */
export function resetsLabel(seconds: number, now: number): string {
	const day = new Date(seconds * 1000).toLocaleDateString(undefined, { weekday: 'short' });
	return seconds - now < 86_400
		? `Resets at ${shortTime(seconds)}`
		: `Resets ${day} ${shortTime(seconds)}`;
}

/** A plan limit's label without the word "limit", which a list of limits implies: "Weekly limit" is "Weekly". */
export function shortLimitLabel(label: string): string {
	const short = label
		.replace(/\blimits?\b/gi, '')
		.replace(/\s+/g, ' ')
		.replace(/^[\s,.:;·–—-]+|[\s,.:;·–—-]+$/g, '');
	return short || label;
}

/** The plan limits an actor draws on: those of the pool listing it, which several actors may share. None when no report names it. */
export function planOf(usage: ListedActorUsage[], actorId: number): PlanUsage[] {
	return usage.find((u) => u.actor_ids.includes(actorId))?.plan ?? [];
}
