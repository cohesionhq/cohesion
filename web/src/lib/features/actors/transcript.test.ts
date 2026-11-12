import { describe, expect, it } from 'vitest';
import type {
	ActorSession,
	ActorSessionMessage,
	ActorSessionMessageContent
} from '$lib/protocol/types';
import {
	answersByRequest,
	duration,
	launchedBy,
	shownMessages,
	spacing,
	unplaced
} from './transcript';

const message = (
	id: number,
	sent_at: number,
	content: ActorSessionMessageContent
): ActorSessionMessage => ({
	id,
	session_id: 1,
	sent_at,
	content,
	answered: false,
	attachment_count: 0
});

const user = (text: string): ActorSessionMessageContent => ({ sender: 'user', kind: 'text', text });
const actor = (text: string): ActorSessionMessageContent => ({
	sender: 'actor',
	kind: 'text',
	text
});
const tool = (ended_at: number | null): ActorSessionMessageContent => ({
	sender: 'actor',
	kind: 'toolUse',
	tool: 'Bash',
	summary: 'ls',
	category: 'command',
	ended_at
});
const thinking = (ended_at: number | null): ActorSessionMessageContent => ({
	sender: 'actor',
	kind: 'thinking',
	text: 'Hm',
	ended_at
});
const answers = (request_id: number, picked: string): ActorSessionMessageContent => ({
	sender: 'user',
	kind: 'answers',
	request_id,
	answers: [{ selected: [picked] }]
});

/** A subagent launched by tool use `message_id`, or by none shown. */
const subagent = (id: number, message_id: number | null) =>
	({ id, subagent: { message_id } }) as ActorSession;

describe('a transcript’s answers', () => {
	const transcript = [
		message(1, 0, user('Go')),
		message(2, 1, actor('Which?')),
		message(3, 2, answers(2, 'This one')),
		message(4, 3, actor('Done'))
	];

	it('are held by their question, not drawn as rows', () => {
		expect(shownMessages(transcript).map((m) => m.id)).toEqual([1, 2, 4]);
		expect(answersByRequest(transcript).get(2)).toEqual([{ selected: ['This one'] }]);
		expect(answersByRequest(transcript).has(4)).toBe(false);
	});
});

describe('a session’s subagents', () => {
	const messages = [message(1, 0, user('Go')), message(2, 1, tool(5)), message(3, 6, actor('Ok'))];

	it('stand in for the tool use that launched them', () => {
		const launched = launchedBy([subagent(10, 2), subagent(11, null)]);
		expect(launched.get(2)?.id).toBe(10);
		expect(launched.size).toBe(1);
	});

	it('follow the transcript when no tool use shown launched them', () => {
		// 11 names no message, 12 one that isn't loaded, 13 one that isn't a tool use.
		const all = [subagent(10, 2), subagent(11, null), subagent(12, 99), subagent(13, 3)];
		expect(unplaced(all, messages).map((s) => s.id)).toEqual([11, 12, 13]);
	});
});

describe('how long a tool or thinking took', () => {
	it('runs until its actor said it ended', () => {
		const messages = [message(1, 100, tool(107)), message(2, 300, actor('Ok'))];
		expect(duration(messages, 0, 'working', 500)).toBe('7s');
	});

	it('counts up while it runs', () => {
		const messages = [message(1, 100, thinking(null))];
		expect(duration(messages, 0, 'working', 112)).toBe('12s');
		// A clock behind the daemon's doesn't count below nothing, and a step under a second shows no time.
		expect(duration(messages, 0, 'working', 90)).toBeNull();
	});

	it('runs until the next message when its actor never said', () => {
		const messages = [message(1, 100, tool(null)), message(2, 130, actor('Ok'))];
		expect(duration(messages, 0, 'idle', 500)).toBe('30s');
	});

	it('is unknown for the last one of a session that stopped', () => {
		expect(duration([message(1, 100, tool(null))], 0, 'idle', 500)).toBeNull();
	});
});

describe('the space above a row', () => {
	const messages = [
		message(1, 0, user('Go')),
		message(2, 1, tool(2)),
		message(3, 2, thinking(3)),
		message(4, 3, actor('Ok')),
		message(5, 4, actor('And'))
	];

	it('is none for the first', () => {
		expect(spacing(messages, 0)).toBe(false);
	});

	it('is wide on a change of speaker', () => {
		expect(spacing(messages, 1)).toBe('mt-6');
	});

	it('is none between one-line rows', () => {
		expect(spacing(messages, 2)).toBe('mt-0');
	});

	it('is small between the same speaker’s other rows', () => {
		expect(spacing(messages, 3)).toBe('mt-3');
		expect(spacing(messages, 4)).toBe('mt-3');
	});
});
