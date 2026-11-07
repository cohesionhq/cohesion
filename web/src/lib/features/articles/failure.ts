// How an article's failed fetch reads, as a sentence for people.

import type { ArticleFetchFailure } from '$lib/protocol/types';

/** Why the page couldn't be fetched, as a sentence. */
export function failureSentence(failure: ArticleFetchFailure): string {
	switch (failure.kind) {
		case 'unreachable':
			return 'The site didn’t answer. The address may be wrong, or the site may be down.';
		case 'status':
			return `The site answered with error ${failure.code}.`;
		case 'unsupportedContent':
			return 'The address isn’t a web page or plain text.';
		case 'tooLarge':
			return 'The page is too large to save. The limit is 5 MB.';
		case 'notAFeed':
			return 'The address isn’t an RSS or Atom feed.';
	}
}

/** The address's host without a leading `www.`, or the address itself if it doesn't parse. */
export function hostOf(url: string): string {
	try {
		return new URL(url).hostname.replace(/^www\./, '');
	} catch {
		return url;
	}
}
