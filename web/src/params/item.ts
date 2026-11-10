import type { ParamMatcher } from '@sveltejs/kit';
import type { ItemKind } from '$lib/shell/routes';

/** The kinds of item a `cohesion://` link opens. */
export const match = ((param: string): param is ItemKind =>
	param === 'task' ||
	param === 'document' ||
	param === 'agent' ||
	param === 'actor-session' ||
	param === 'article') satisfies ParamMatcher;
