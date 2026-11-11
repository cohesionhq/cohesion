// A session's transcript as its page draws it (`SessionView.svelte`): which
// messages show as rows, the answers a question's card holds, where each
// subagent's line goes, how long a tool or thinking took, and how far a row
// sits from the one before.

import { elapsed } from '$lib/format/dates';
import type {
	ActorAnswer,
	ActorSession,
	ActorSessionMessage,
	ActorSessionStatus
} from '$lib/protocol/types';
import { stillRunning } from './tools';

const isAnswers = ({ content }: ActorSessionMessage) =>
	content.sender === 'user' && content.kind === 'answers';

/** The user's answers to each question, by the question's id; they show in its card. */
export function answersByRequest(transcript: ActorSessionMessage[]): Map<number, ActorAnswer[]> {
	return new Map(
		transcript.flatMap(({ content }): [number, ActorAnswer[]][] =>
			content.sender === 'user' && content.kind === 'answers'
				? [[content.request_id, content.answers]]
				: []
		)
	);
}

/** The transcript as the feed draws it, without the answers its question cards hold. */
export function shownMessages(transcript: ActorSessionMessage[]): ActorSessionMessage[] {
	return transcript.filter((message) => !isAnswers(message));
}

/** Each subagent by the tool use that launched it, which its line stands in for. */
export function launchedBy(subagents: ActorSession[]): Map<number, ActorSession> {
	return new Map(
		subagents.flatMap((s): [number, ActorSession][] =>
			s.subagent?.message_id != null ? [[s.subagent.message_id, s]] : []
		)
	);
}

/** The subagents no tool use among `messages` launched, which follow the transcript. */
export function unplaced(
	subagents: ActorSession[],
	messages: ActorSessionMessage[]
): ActorSession[] {
	return subagents.filter(
		(s) => !messages.some((m) => m.id === s.subagent?.message_id && m.content.kind === 'toolUse')
	);
}

/**
 * How long the tool or thinking at `index` took, in whole seconds: until
 * its actor said it ended, counting up to `now` while it runs in a session
 * standing as `status`. One whose actor never said (an older one, or one the
 * session stopped in) is taken to run until the next message, and is unknown
 * if it was the last.
 */
export function duration(
	messages: ActorSessionMessage[],
	index: number,
	status: ActorSessionStatus | undefined,
	now: number
): string | null {
	const said = messages[index].content;
	const start = messages[index].sent_at;
	if (said.sender === 'actor' && 'ended_at' in said && said.ended_at !== null)
		return elapsed(said.ended_at - start);
	if (stillRunning(status, said)) return elapsed(Math.max(0, now - start));
	const next = messages[index + 1];
	return next ? elapsed(next.sent_at - start) : null;
}

/** Whether `message` is a one-line row: a tool the actor ran, or its folded thinking. */
const isLine = (message: ActorSessionMessage | undefined) =>
	message !== undefined && ['toolUse', 'thinking'].includes(message.content.kind);

/**
 * How far the message at `index` sits from what came before, as a margin
 * class: wide on a change of speaker, none between one-line rows.
 */
export function spacing(messages: ActorSessionMessage[], index: number): string | false {
	if (index === 0) return false;
	if (messages[index].content.sender !== messages[index - 1].content.sender) return 'mt-6';
	if (isLine(messages[index]) && isLine(messages[index - 1])) return 'mt-0';
	return 'mt-3';
}
