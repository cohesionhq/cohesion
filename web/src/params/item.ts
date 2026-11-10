import type { ParamMatcher } from '@sveltejs/kit';
import type { UnscopedItemKind } from '$lib/shell/routes';

/** The kinds of item a `cohesion://` link opens. */
export const match = ((param: string): param is UnscopedItemKind =>
	param === 'task' ||
	param === 'document' ||
	param === 'agent' ||
	param === 'actor-session' ||
	param === 'article') satisfies ParamMatcher;
