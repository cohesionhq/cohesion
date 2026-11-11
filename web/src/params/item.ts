import type { ParamMatcher } from '@sveltejs/kit';
import { isUnscopedItemKind } from '$lib/format/itemLinks';

/** The kinds of item a `cohesion://` link opens. */
export const match = isUnscopedItemKind satisfies ParamMatcher;
