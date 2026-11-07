import { describe, expect, it } from 'vitest';
import { workingCaption } from './workingCaption';

describe('workingCaption', () => {
	it("says what the daemon's activity says, trailing off", () => {
		expect(workingCaption('Thinking')).toBe('Thinking…');
		expect(workingCaption('Running tools/fetch.py')).toBe('Running tools/fetch.py…');
	});

	it('falls back to Working… with no activity or an empty one', () => {
		for (const activity of [undefined, null, '', '   ', '…']) {
			expect(workingCaption(activity)).toBe('Working…');
		}
	});

	it('gives way to the ellipsis when a title ends in its own', () => {
		expect(workingCaption('Fetching the article...')).toBe('Fetching the article…');
		expect(workingCaption('Fetching the article… ')).toBe('Fetching the article…');
		expect(workingCaption('Fetching the article.')).toBe('Fetching the article…');
	});
});
