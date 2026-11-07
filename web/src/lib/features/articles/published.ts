// When an article was published, as its precision allows: a day is shown as
// that date in every time zone, never shifted by the reader's.

import { longDate, longDay, relativeDay, relativeTime, utcDay } from '$lib/format/dates';
import type { ArticleSummary } from '$lib/protocol/types';

type Published = Pick<ArticleSummary, 'published_at' | 'published_precision'>;

/** A compact age for a list: "5m", "3h", "2d"; a day's counts calendar days. */
export function publishedAge(article: Published, now: number): string {
	return article.published_precision === 'day'
		? relativeDay(utcDay(article.published_at), now)
		: relativeTime(article.published_at, now);
}

/** "Monday, October 12, 2026" */
export function publishedDate(article: Published): string {
	return article.published_precision === 'day'
		? longDay(utcDay(article.published_at))
		: longDate(article.published_at);
}
