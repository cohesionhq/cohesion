import { describe, expect, it } from 'vitest';
import { mentionTrigger } from './mentionTrigger';

describe('mention trigger', () => {
	it('opens on @ at a word start, once a character follows', () => {
		expect(mentionTrigger('@')).toBeNull();
		expect(mentionTrigger('@f')).toEqual({ at: 0, query: 'f' });
		expect(mentionTrigger('see @fix log')).toEqual({ at: 4, query: 'fix log' });
		expect(mentionTrigger('mail@home')).toBeNull();
	});

	it('opens on [[ the same way', () => {
		expect(mentionTrigger('[[')).toBeNull();
		expect(mentionTrigger('[[f')).toEqual({ at: 0, query: 'f' });
		expect(mentionTrigger('see [[fix log')).toEqual({ at: 4, query: 'fix log' });
		expect(mentionTrigger('- [[fix')).toEqual({ at: 2, query: 'fix' });
		expect(mentionTrigger('a[[fix')).toBeNull();
		expect(mentionTrigger('[[ fix')).toBeNull();
	});

	it('closes once a [[ search meets a bracket', () => {
		expect(mentionTrigger('[[Fix]')).toBeNull();
		expect(mentionTrigger('[[Fix]]')).toBeNull();
		expect(mentionTrigger('[[[a')).toBeNull();
	});

	it('answers to the opener typed last', () => {
		expect(mentionTrigger('@fix [[log')).toEqual({ at: 5, query: 'log' });
		expect(mentionTrigger('[[fix @log')).toEqual({ at: 6, query: 'log' });
	});
});
