// Generated from cohesion-core by `npm run generate`; do not edit.

export const PROTOCOL_VERSION = 1;

/** Marks a request that carries no `args` key on the wire. */
export type NoArgs = undefined;

export interface AboutResponse {
	hostname: string;
	version: string;
}

export type AcceptedResponse = null;

/**
 * What serves an account's items. The table's checks give every row
 * exactly one of these.
 */
export type AccountHost =
	| {
			kind: 'extension';
			/** Id of the extension that serves the account. */
			extension_id: number;
			/** The account's identifier at the service, unique per extension. */
			identifier: string;
	  }
	| { kind: 'orphaned' };

/**
 * A unit of long-running work Cohesion shows the user while it goes on and
 * keeps once it ends: its title, what it is, where it stands and, while it
 * runs, a short line on its progress. The work itself is owned elsewhere
 * (an actor's session); only `ActivityTracker` writes these rows.
 */
export interface Activity {
	id: number;
	title: string;
	/** What the activity is, and so where a client jumps to show it. */
	kind: ActivityKind;
	status: ActivityStatus;
	/**
	 * What the work last said it is doing, such as "Read 12 of 40
	 * sources"; `null` when it said nothing.
	 */
	progress: string | null;
	started_at: number;
	/** When the activity ended; `null` while it runs or waits. */
	ended_at: number | null;
}

/** What an activity is. */
export type ActivityKind = {
	kind: 'actorSession';
	/** The session it shows. */
	actor_session_id: number;
};

/**
 * Where an activity stands.
 *
 * - `"running"`, `"waiting"`, `"paused"`: going on, as an
 *   [`ActivityStanding`] says.
 * - `"succeeded"`, `"failed"`, `"cancelled"`: ended, as an
 *   [`ActivityOutcome`] says.
 */
export type ActivityStatus =
	'running' | 'waiting' | 'paused' | 'succeeded' | 'failed' | 'cancelled';

/**
 * An agent on a client's device, such as Claude Code in one directory,
 * that Cohesion hands sessions to: a client offers each in its `Hello`, and
 * works each session in the actor's own system, reporting back what it says
 * and where it stands.
 *
 * An actor has no name of its own: every read takes its client's name
 * (a join of `clients`), so renaming the client renames its actors. One
 * its client no longer offers is soft-deleted, keeping its sessions.
 */
export interface Actor {
	id: number;
	deleted: boolean;
	/** The client that offers it, and whose online state it shares. */
	client_id: number;
	/** What its client calls it, unique among the client's actors. */
	identifier: string;
	/** The agent's product, such as "Claude Code". */
	product: string;
	/** Where it works, such as a directory: `/Users/ada/Developer/cohesion`. */
	scope: string;
	/** How it can run a session, the first the default. */
	modes: ActorMode[];
	/** The models it can run a session on, the first the default. */
	models: ActorModel[];
	/**
	 * The product on its client's current name, such as "Claude Code on
	 * Ada's MacBook Pro".
	 */
	title: string;
}

/** The answer to one question of an actor's. */
export interface ActorAnswer {
	/** The labels of the options picked. */
	selected: string[];
	/** The answer in the user's own words, when the question allows it. */
	other?: string | null;
}

/** A reasoning effort an actor's model takes. */
export interface ActorEffort {
	identifier: string;
	/** The effort as people know it, such as "High". */
	title: string;
}

/** One file an actor asks to change. */
export interface ActorFileChange {
	path: string;
	change: ActorFileChangeKind;
	/** The change as a unified diff, when the actor has one. */
	diff: string | null;
}

/** How a file changes. */
export type ActorFileChangeKind = 'add' | 'update' | 'delete';

/**
 * What an actor says in a session, tagged by `kind`.
 *
 * - `"text"`: the agent's own words, as Markdown.
 * - `"thinking"`: its reasoning on the way, as Markdown: a summary or the
 *   full chain, whichever its client has, and when it ended. Clients
 *   show it quieter than its words.
 * - `"toolUse"`: a tool it ran, such as editing a file, and when it ended.
 * - `"question"`: questions for the user, each with options (Claude Code's
 *   `AskUserQuestion`, Codex's `request_user_input`).
 * - `"permission"`: something it needs the user's leave to do: run a
 *   command, change files, or use a tool.
 * - `"plan"`: a plan it asks the user to approve before it starts
 *   (Claude Code's `ExitPlanMode`).
 *
 * The last three are requests: the session waits for the user's response.
 */
export type ActorMessageContent =
	| {
			kind: 'text';
			text: string;
	  }
	| {
			kind: 'thinking';
			text: string;
			/**
			 * When it ended, in Unix seconds, as Cohesion heard of it from an
			 * `end` update; `null` while it goes on, and for thinking whose
			 * actor never said.
			 */
			ended_at: number | null;
	  }
	| {
			kind: 'toolUse';
			/** The tool, such as "Edit", as the actor names it. */
			tool: string;
			/**
			 * What kind of tool it is, as the actor's client classifies it;
			 * `"other"` when left out.
			 */
			category?: ActorToolKind;
			/** What it did, in a line, such as "src/main.rs". */
			summary: string;
			/**
			 * When it ended, in Unix seconds, as Cohesion heard of it from an
			 * `end` update; `null` while it runs, and for one whose actor
			 * never said (a tool it was stopped in, or an actor that doesn't
			 * report ends).
			 */
			ended_at: number | null;
	  }
	| {
			kind: 'question';
			questions: ActorQuestion[];
	  }
	| ({ kind: 'permission' } & ActorPermissionRequest)
	| {
			kind: 'plan';
			/** The plan, as Markdown. */
			plan: string;
	  };

/**
 * One way an actor can run a session, such as sandboxed or accepting edits on
 * its own.
 */
export interface ActorMode {
	identifier: string;
	/** The mode as people know it, such as "Auto". */
	title: string;
	/** What it lets the actor do without asking. */
	description: string | null;
}

/** A model an actor can run a session on, and how hard it can make it think. */
export interface ActorModel {
	identifier: string;
	/** The model as people know it, such as "Opus". */
	title: string;
	/**
	 * The reasoning efforts it takes, least first; none when it takes no
	 * setting.
	 */
	efforts: ActorEffort[];
}

/**
 * What the user decides about an actor's permission request.
 *
 * - `"allow"`: this once.
 * - `"allowForSession"`: this and the like for the rest of the session.
 * - `"deny"`: not this; the actor carries on without it.
 * - `"abort"`: not this, and stop what it is doing.
 */
export type ActorPermissionDecision = 'allow' | 'allowForSession' | 'deny' | 'abort';

/**
 * Something an actor needs the user's leave to do, and the decisions it
 * takes.
 */
export interface ActorPermissionRequest {
	subject: ActorPermissionSubject;
	/** Why it wants to, in its words. */
	reason: string | null;
	/** The decisions it takes, in the order to offer them. */
	decisions: ActorPermissionDecision[];
}

/**
 * What an actor asks leave to do, tagged by `kind`.
 *
 * - `"command"`: run a shell command.
 * - `"fileChange"`: change files.
 * - `"tool"`: use any other tool, with its input as JSON.
 */
export type ActorPermissionSubject =
	| {
			kind: 'command';
			command: string;
			/** The directory it runs in. */
			cwd: string | null;
	  }
	| {
			kind: 'fileChange';
			changes: ActorFileChange[];
	  }
	| {
			kind: 'tool';
			name: string;
			input: unknown;
	  };

/** One question an actor asks. */
export interface ActorQuestion {
	/** A short label for it, such as "Auth method". */
	header: string | null;
	question: string;
	/** The answers to pick from; none for a question answered in words. */
	options: ActorQuestionOption[];
	/** Whether more than one option may be picked. */
	multi_select: boolean;
	/** Whether the user may answer in their own words instead. */
	allows_other: boolean;
}

/** An answer to pick for an actor's question. */
export interface ActorQuestionOption {
	label: string;
	/** What picking it means. */
	description: string | null;
}

/**
 * One session an actor works on, such as a Claude Code session: a thread in
 * the actor's own system, which it reports to Cohesion as it goes. While
 * it works, or needs the user, an activity shows it. A session stays until the
 * user, or an agent for them, resolves it, and never ends for good: a
 * message to one that is done, failed or resolved starts it again.
 */
export interface ActorSession {
	id: number;
	actor_id: number;
	title: string;
	/** The actor's mode it runs in; `null` for the actor's default. */
	mode: string | null;
	/** The actor's model it runs on; `null` for the actor's default. */
	model: string | null;
	/** The model's reasoning effort; `null` for the model's default. */
	effort: string | null;
	started_at: number;
	/** When its status last changed, which orders the sessions. */
	status_changed_at: number;
	/** Where it stands, as its actor last reported. */
	status: ActorSessionStatus;
	/**
	 * Whether its actor finished a stretch of work, done or failed, since
	 * the user last read it; stopping it, or its client going away, doesn't.
	 */
	unread: boolean;
	/** When the user last read it; `null` if they never have. */
	read_at: number | null;
	/**
	 * How full its context window is, as its actor last reported; `null`
	 * until it says.
	 */
	context: ContextUsage | null;
	/**
	 * The agent following it, the last to tell it something, which hears
	 * each time it stops on its own; `null` when none does.
	 */
	follower: ActorSessionFollower | null;
	/**
	 * What makes it a subagent: the session one level up whose actor
	 * launched it, and how; `null` for a session Cohesion started. A
	 * subagent leaves the lists, has no activity, is never unread and wakes
	 * no follower; it goes with the session above it.
	 */
	subagent: ActorSubagent | null;
}

/**
 * The agent following an actor session, and who started the chain of work that
 * told the session something, which the follower's fork inherits.
 */
export interface ActorSessionFollower {
	agent_id: number;
	initiative: Initiative;
}

/** One entry in an actor session's transcript. */
export interface ActorSessionMessage {
	id: number;
	session_id: number;
	sent_at: number;
	/**
	 * Who said it and what, stored as `sender`, `kind` and the columns they
	 * set.
	 */
	content: ActorSessionMessageContent;
	/**
	 * Whether the user has answered it; only a request can be. A request
	 * is answered by its response, or passed over by anything the user
	 * says after it, which the actor takes as their reply instead.
	 */
	answered: boolean;
	/**
	 * How many images came with it, the user's or the actor's;
	 * `ListActorSessionMessageAttachments` fetches them.
	 */
	attachment_count: number;
}

/**
 * Who said an actor session message, tagged by `sender`, and what it says,
 * tagged by `kind`.
 */
export type ActorSessionMessageContent =
	({ sender: 'actor' } & ActorMessageContent) | ({ sender: 'user' } & ActorUserContent);

/**
 * An actor session matching a full-text search of the sessions' titles and
 * what the user and the actor said in them, with the passage that
 * matched. Read only by the search, which joins each match as `hit`.
 */
export interface ActorSessionSearchHit {
	id: number;
	actor_id: number;
	title: string;
	status_changed_at: number;
	/**
	 * A few words of the message that matched, in Markdown; `null` when
	 * the title matched best.
	 */
	excerpt: string | null;
}

/**
 * Where an actor session stands.
 *
 * - `"working"`: the actor is at it.
 * - `"needsAttention"`: the actor is waiting on the user, such as for a
 *   response to a request.
 * - `"idle"`: done, until the user says more.
 * - `"failed"`: the actor couldn't go on; the user may say more to try
 *   again.
 * - `"resolved"`: the user, or an agent for them, is done with it; lists
 *   of sessions leave it out, and it is deleted once it has sat resolved
 *   for the actor session retention. The user may say more to start it
 *   again.
 *
 * A working session, or one that needs attention, has an activity open, which
 * ends succeeded as it goes idle and failed as it fails.
 */
export type ActorSessionStatus = 'working' | 'needsAttention' | 'idle' | 'failed' | 'resolved';

/**
 * How an actor session stopped on its own: each wakes the agent following it.
 *
 * - `"needsYou"`: the actor waits on the user.
 * - `"finished"`: the actor finished its turn.
 * - `"failed"`: the actor couldn't go on.
 * - `"interrupted"`: the session's device went away mid-turn.
 */
export type ActorSessionStop = 'needsYou' | 'finished' | 'failed' | 'interrupted';

/**
 * A subagent: a session an actor launched from inside another to do part
 * of its work, such as Claude Code's Task tool. Its client reports into it
 * under the session Cohesion started, naming it by `key`.
 */
export interface ActorSubagent {
	/**
	 * The session one level up: the one Cohesion started, or another
	 * subagent.
	 */
	parent_session_id: number;
	/** The client's own id for it, unique among its actor's subagents. */
	key: string;
	/**
	 * What kind of subagent the actor launched, as it names them, such as
	 * "Explore"; `null` when it didn't say.
	 */
	agent_type: string | null;
	/**
	 * The tool use in the transcript one level up that launched it; `null`
	 * when the client didn't say.
	 */
	message_id: number | null;
	/**
	 * Whether it runs in the background, going on after the tool use that
	 * launched it has ended.
	 */
	background: boolean;
}

/**
 * What kind of tool an actor ran, which its client decides from the
 * tool's name, since only the client knows its actor's tools.
 *
 * - `"command"`: runs a shell command (Claude Code's `Bash`).
 * - `"read"`: reads a file or directory (`Read`).
 * - `"edit"`: changes files (`Edit`, `Write`).
 * - `"search"`: searches files or their contents (`Grep`, `Glob`).
 * - `"web"`: fetches or searches the web (`WebFetch`, `WebSearch`).
 * - `"agent"`: hands work to another agent (`Agent`).
 * - `"skill"`: takes up a skill, instructions it loads for the session
 *   (`Skill`).
 * - `"other"`: anything else, and a tool its client didn't classify.
 */
export type ActorToolKind =
	'command' | 'read' | 'edit' | 'search' | 'web' | 'agent' | 'skill' | 'other';

/**
 * How much of its plan's limits an actor has used, as its client last
 * reported. A session's context window is its own, which it reports as the
 * session's `context`.
 */
export interface ActorUsage {
	/**
	 * Its plan's limits, such as a five-hour and a weekly one, in the
	 * order to show them. Left out, none.
	 */
	plan?: PlanUsage[];
	/**
	 * The account whose limits these are, when other actors draw on it
	 * too. Left out, the actor's limits are its own.
	 */
	pool?: UsagePool | null;
}

/**
 * What the user says in an actor session, tagged by `kind`.
 *
 * - `"text"`: a message, the first the session's prompt.
 * - `"answers"`: the answers to a `question`, one per question in order.
 * - `"permission"`: the decision on a `permission` request.
 * - `"plan"`: whether a `plan` is approved.
 *
 * Every kind but `text` responds to the request `request_id`, at most once.
 */
export type ActorUserContent =
	| {
			kind: 'text';
			text: string;
	  }
	| {
			kind: 'answers';
			request_id: number;
			answers: ActorAnswer[];
	  }
	| {
			kind: 'permission';
			request_id: number;
			decision: ActorPermissionDecision;
			/** What to tell the actor with it, such as why not. */
			message?: string | null;
	  }
	| {
			kind: 'plan';
			request_id: number;
			approved: boolean;
			/** What to change, when not approved. */
			feedback?: string | null;
	  };

/**
 * Adds an email address or phone number, which may not be empty, to live
 * contact `contact_id` in workspace `workspace_id`, after its others.
 */
export interface AddContactMethodRequest {
	workspace_id: number;
	contact_id: number;
	kind: ContactMethodKind;
	value: string;
}

/**
 * Makes endpoint `endpoint_id` offer model `model_identifier`, linked to
 * the catalog when it lists the model for the endpoint's provider. The
 * request is the whole model: the fields given override the catalog's, and
 * one left out falls back to the catalog's (`model_name` then to the
 * identifier), clearing any override. Adding a model the endpoint already
 * offers replaces it.
 */
export interface AddEndpointModelRequest {
	endpoint_id: number;
	model_identifier: string;
	model_name?: string | null;
	context_size?: number | null;
	prices?: ModelPrices | null;
}

/**
 * Reads the feed at `url` (http or https) and follows it, last in the
 * order, adding its items to the feed corpus.
 */
export interface AddFeedRequest {
	url: string;
	/** `"whenExcerpt"` when absent. */
	full_page_policy?: FeedFullPagePolicy;
}

/**
 * One agent's question to another, stored twice: as `askedAgent` in the
 * conversation of the agent that asked, and as `askedByAgent` in the
 * conversation of the agent that answered, each naming the other in
 * `agent`.
 */
export interface AgentAsk {
	/**
	 * The other agent: the one asked in `askedAgent`, the one asking in
	 * `askedByAgent`.
	 */
	agent: AgentPeer;
	question: string;
	reply: AgentAskReply;
}

/**
 * Where a question to another agent stands, tagged by `status`; both of
 * its messages always say the same.
 *
 * - `"waiting"`: the agent asked is still answering.
 * - `"answered"`: it answered with `text`.
 * - `"unanswered"`: no answer came, for `reason`, such as the wait timing
 *   out.
 */
export type AgentAskReply =
	| { status: 'waiting' }
	| {
			status: 'answered';
			text: string;
	  }
	| {
			status: 'unanswered';
			reason: string;
	  };

/**
 * A question the agent asks with answers to pick from, lettered `A`, `B`,
 * `C` and so on, stored as a `choice` message. Picking one records it here
 * and starts the agent's turn on it; the pick posts no message of its own.
 */
export interface AgentChoice {
	/** The question, in Markdown. */
	question: string;
	/** The answers on offer, in letter order. */
	options: string[];
	/**
	 * From a background run: what the agent's conversation needs to act on
	 * the pick, such as what the run found and what each answer leads to.
	 * The agent reads it with the pick; the user doesn't see it. `null`
	 * from the conversation itself.
	 */
	context: string | null;
	/** The index in `options` of the user's pick; `null` until they pick. */
	selected: number | null;
}

/** A script the agent ran, stored as a `codeExecution` message. */
export interface AgentCodeExecution {
	/** What the script does, as the agent described it. */
	title: string;
	code: string;
	/** How the script ended. */
	outcome: AgentCodeOutcome;
}

/**
 * How a script the agent ran ended, tagged by `status`.
 *
 * - `"succeeded"`: it ran to the end, returning `result`.
 * - `"failed"`: it threw or ran out of time, for `error`.
 * - `"cancelled"`: the user stopped the agent's turn while it ran.
 */
export type AgentCodeOutcome =
	| {
			status: 'succeeded';
			result: string;
	  }
	| {
			status: 'failed';
			error: string;
	  }
	| { status: 'cancelled' };

/**
 * The item a message to an agent forwards, by the kind its `cohesion://`
 * link names it by: a journal entry is a document.
 */
export type AgentForwardTarget =
	| {
			kind: 'task';
			workspace_id: number;
			id: number;
	  }
	| {
			kind: 'document';
			workspace_id: number;
			id: number;
	  };

/**
 * The item a `forward` message forwards, which the agent reads from its
 * `cohesion://` link with its code tool rather than being sent a copy.
 */
export interface AgentForwardedItem {
	kind: AgentForwardedItemKind;
	workspace_id: number;
	id: number;
	/**
	 * The task's or document's title when it was forwarded; a journal
	 * entry's is its day, `YYYY-MM-DD`.
	 */
	title: string;
}

/** What kind of item a `forward` message forwards. */
export type AgentForwardedItemKind = 'task' | 'document' | 'journal';

/** What became of a memory, in a `memory` message. */
export type AgentMemoryChange = 'remembered' | 'forgotten';

/**
 * What an agent message says, tagged by `kind`.
 *
 * - `"text"`: plain text.
 * - `"codeExecution"`: a script the agent ran.
 * - `"error"`: why the sender's turn or run failed, or (from
 *   `backgroundAgent`, with no `run_id`) why one of its cues' functions did.
 * - `"documentation"`: the script API module the sender loaded, such as
 *   `network`.
 * - `"choice"`: a question with answers to pick from.
 * - `"memory"`: a memory the agent remembered or forgot.
 * - `"forward"`: the user's message forwarding an item, with their note.
 * - `"askedAgent"`: the agent's question to another agent, and its answer.
 * - `"askedByAgent"`: another agent's question to the agent, and the answer
 *   it gave.
 * - `"note"`: what the agent noted for its own later turns, which nobody
 *   is told of.
 * - `"woke"`: why the agent woke apart from its conversation: a cue fired,
 *   or an actor session it follows stopped. What that work posts follows,
 *   marked with its run.
 * - `"rebuiltContext"`: the agent's context was built again from the
 *   database rather than carrying on its last thread, which loses the
 *   provider's cached prompt, and `reason` says why
 *   (`null` on a line written before reasons were kept).
 */
export type AgentMessageContent =
	| {
			kind: 'text';
			text: string;
	  }
	| ({ kind: 'codeExecution' } & AgentCodeExecution)
	| {
			kind: 'error';
			error: string;
	  }
	| {
			kind: 'documentation';
			module: string;
	  }
	| ({ kind: 'choice' } & AgentChoice)
	| {
			kind: 'memory';
			memory: string;
			change: AgentMemoryChange;
	  }
	| {
			kind: 'forward';
			/** What the user wrote with the item, or empty. */
			note: string;
			item: AgentForwardedItem;
	  }
	| ({ kind: 'askedAgent' } & AgentAsk)
	| ({ kind: 'askedByAgent' } & AgentAsk)
	| {
			kind: 'note';
			note: string;
	  }
	| {
			kind: 'woke';
			by: AgentWake;
	  }
	| {
			kind: 'rebuiltContext';
			reason?: ContextRebuildReason | null;
	  };

/**
 * An agent named in another's conversation, as it was named when it was
 * asked or asked.
 */
export interface AgentPeer {
	id: number;
	name: string;
}

/**
 * What a message is to the fork that posted it, which says where a client
 * shows it: a fork's messages are told apart by `run_id`, and may fall among
 * the conversation's own or another fork's.
 *
 * - `"trigger"`: what woke the fork, its `woke` line or another agent's
 *   question to it. A client collapses the fork's steps under it.
 * - `"step"`: work the fork did on the way: a script, a module it loaded, a
 *   memory, a note, a question to another agent. It belongs under the
 *   trigger, not in the main transcript.
 * - `"surfaced"`: what the fork has to show the user: a text, a question to
 *   pick an answer to, or why it failed. It stays in the main transcript.
 *
 * An older fork may have no trigger, from before wake lines.
 */
export type AgentRunPart = 'trigger' | 'step' | 'surfaced';

/** What woke an agent apart from its conversation, in a `woke` message. */
export type AgentWake =
	| {
			kind: 'cue';
			cue_name: string;
			hook: HookKind;
	  }
	| {
			kind: 'actorSession';
			actor_session_id: number;
			title: string;
			stop: ActorSessionStop;
	  }
	| {
			kind: 'assignedTask';
			workspace_id: number;
			task_id: number;
			title: string;
	  };

/**
 * Appends `content` to the journal for `date` (`"YYYY-MM-DD"`) in a
 * workspace, creating it if needed.
 */
export interface AppendToJournalRequest {
	workspace_id: number;
	date: string;
	content: string;
}

/**
 * A web page a feed brought in, with its content as Markdown as of when it
 * was last fetched. Articles belong to no workspace: they make up the feed
 * corpus every workspace shares.
 */
export interface Article {
	/** Unique, stable identifier. */
	id: number;
	/**
	 * Whether the article has been soft-deleted. Listings skip deleted articles,
	 * so this is normally `false`.
	 */
	deleted: boolean;
	/** The feed it came from. */
	feed_id: number;
	/** Whether it has been read. A new article starts unread. */
	read: boolean;
	/** The item's link in the feed. */
	url: string;
	/**
	 * Display name: the item's title in the feed, else its page's, else the
	 * last part of the URL's path (its host when the path is empty), until
	 * renamed.
	 */
	title: string;
	/** Who wrote it, when the feed or page says. */
	author: string | null;
	/**
	 * When it was published, in Unix seconds: the date the feed or page
	 * gives, else when it was brought in.
	 */
	published_at: number;
	/** Whether `published_at` is a moment or only a day. */
	published_precision: PublishedPrecision;
	/** How the last fetch went. */
	snapshot: ArticleSnapshot;
	/**
	 * The page's main content as Markdown (a plain-text page as it was
	 * served) as of the snapshot; `""` when the page has never been
	 * fetched successfully. Full-text searchable, with the title and
	 * author, through the corpus search.
	 */
	contents: string;
	/** When the article was brought in, in Unix seconds. */
	created_at: number;
	/** When the article last changed, in Unix seconds. */
	modified: number;
}

/** Why a page couldn't be fetched. */
export type ArticleFetchFailure =
	| {
			kind: 'unreachable';
			message: string;
	  }
	| {
			kind: 'status';
			code: number;
	  }
	| {
			kind: 'unsupportedContent';
			content_type: string;
	  }
	| { kind: 'tooLarge' }
	| { kind: 'notAFeed' };

/**
 * Which articles a listing covers.
 *
 * - `{ "feed": id }`: every article that came from that feed.
 * - `"feeds"`: every article that came from any feed.
 */
export type ArticleFilter = { feed: number } | 'feeds';

/** One page of a listing: its articles, and how many the whole listing has. */
export interface ArticlePage {
	articles: ArticleSummary[];
	/** Every article the listing covers, on any page. */
	total: number;
}

/** How an article's page was last fetched. */
export type ArticleSnapshot =
	| {
			state: 'captured';
			/** The address the page was read from, after redirects. */
			final_url: string;
			/** The page's own `<title>`, if it had one. */
			page_title: string | null;
			/** When the page was fetched, in Unix seconds. */
			fetched_at: number;
			/** Whether the text was cut short because the page was too long. */
			truncated: boolean;
	  }
	| {
			state: 'failed';
			/** Why the last attempt failed. */
			failure: ArticleFetchFailure;
			/** When it was attempted, in Unix seconds. */
			attempted_at: number;
	  };

/** An article as a listing shows it: everything but its contents. */
export interface ArticleSummary {
	id: number;
	deleted: boolean;
	/** The feed it came from. */
	feed_id: number;
	/** Whether it has been read. */
	read: boolean;
	url: string;
	title: string;
	author: string | null;
	/** In Unix seconds. */
	published_at: number;
	published_precision: PublishedPrecision;
	snapshot: ArticleSnapshot;
	/** In Unix seconds. */
	created_at: number;
	/** In Unix seconds. */
	modified: number;
}

/** What marking a view read changed. */
export interface ArticlesMarkedRead {
	/** How many articles were unread and now are read. */
	marked: number;
}

/**
 * An image with a message: one the user attached, or one an agent or actor
 * showed.
 */
export interface Attachment {
	id: number;
	/** The message it came with. */
	message: AttachmentMessage;
	/** `image/png`, `image/jpeg`, `image/gif` or `image/webp`. */
	media_type: string;
	/** The image, base64-encoded. */
	data: string;
}

/**
 * The message an attachment came with, by which conversation it is in. Real
 * foreign keys, so a message's attachments cascade with it.
 *
 * - `"actorSession"`: a message in an actor session.
 * - `"agent"`: a message in an agent's conversation.
 */
export type AttachmentMessage =
	| {
			kind: 'actorSession';
			actor_session_message_id: number;
	  }
	| {
			kind: 'agent';
			agent_message_id: number;
	  };

/** Asks `oauth` extension `extension_id` where the user signs in. */
export interface BeginConnectorAuthenticationRequest {
	extension_id: number;
}

/** One element of a CohesionUI interface, told apart by `kind`. */
export type Block =
	| ({ kind: 'text' } & TextBlock)
	| ({ kind: 'checkbox' } & CheckboxBlock)
	| ({ kind: 'overview' } & OverviewBlock)
	| ({ kind: 'heading' } & HeadingBlock)
	| ({ kind: 'link' } & LinkBlock);

/**
 * Calls the global `serviceRequest(name, args, caller)` of the extension
 * whose manifest declares `identifier`, which the extension needs the
 * `service` or `service:isolated` permission enabled for, and answers what
 * it returns. `caller` is `{ clientId }`, naming the client asking. An
 * extension that declares a `service` block answers only the names it
 * declares.
 */
export interface CallExtensionRequest {
	/**
	 * The extension's manifest `identifier`, such as
	 * `com.cohesion.cloud.ios-companion`; 1 to 256 characters.
	 */
	identifier: string;
	/**
	 * Which request, as the extension names them; at most 128 characters.
	 * One of its declared functions when it declares a `service` block.
	 */
	name: string;
	/**
	 * Passed to the extension as the JS value this JSON parses to; `null`
	 * when left out. At most 64 KB as JSON.
	 */
	args?: unknown;
}

/** An LLM offered by a provider, with its context size and per-token pricing. */
export interface CatalogModel {
	id: number;
	/** The models.dev provider identifier, for example `openai`. */
	provider_identifier: string;
	/** The provider's display name, for example `OpenAI`. Display only. */
	provider_name: string;
	/** The model identifier, for example `gpt-5`. */
	model_identifier: string;
	/** The model's display name, for example `GPT-5`. Display only. */
	model_name: string;
	context_size: number | null;
	input_token_cost: number | null;
	output_token_cost: number | null;
	cache_read_token_cost: number | null;
	cache_write_token_cost: number | null;
	large_context_threshold: number | null;
	large_context_input_token_cost: number | null;
	large_context_output_token_cost: number | null;
	large_context_cache_read_token_cost: number | null;
	large_context_cache_write_token_cost: number | null;
}

/**
 * Change task's status. Responds with only the tasks whose status changed;
 * tasks already in `status` are left untouched.
 */
export interface ChangeTasksStatusRequest {
	workspace_id: number;
	task_ids: number[];
	status: TaskStatus;
}

/**
 * `kind: "checkbox"`: a yes or no, shown as a checkbox the user can't
 * change.
 */
export interface CheckboxBlock {
	/** Whether it is ticked. */
	checked: boolean;
	/**
	 * A label beside it, such as a to-do's text. Leave it out inside an
	 * overview, whose title labels it.
	 */
	text?: string | null;
}

/** Removes the default model of `tier`. */
export interface ClearIntelligenceDefaultRequest {
	tier: IntelligenceTier;
}

/** Removes the spend limit of `period`, if it has one. */
export interface ClearIntelligenceSpendLimitRequest {
	period: SpendPeriod;
}

/**
 * A client trusted to talk to cohesiond. Any client may show Cohesion to a
 * person, offer actors, or both.
 */
export interface Client {
	id: number;
	deleted: boolean;
	/** What identifies the client, and so what `key` is the fingerprint of. */
	key_kind: ClientKeyKind;
	/** The SHA-256 of what identifies the client; unique among live clients. */
	key: string;
	/** How the client is listed; it can be renamed. */
	client_name: string;
	/**
	 * When it paired, in Unix seconds; `None` for a client paired before
	 * this was recorded.
	 */
	created_at: number | null;
	/**
	 * When it last connected, in Unix seconds; `None` until it does. A
	 * connected client is not updated while its connection lasts.
	 */
	last_seen_at: number | null;
	/**
	 * The address it last connected from; `None` for a client on the
	 * same computer as Cohesion, or one not yet seen.
	 */
	last_address: string | null;
}

/**
 * How a client proves who it is.
 *
 * - `"certificate"`: the certificate it registered with a setup code,
 *   presented over mutual TLS.
 * - `"local"`: the local key it presents on the daemon's Unix socket,
 *   whose file permissions trust it.
 */
export type ClientKeyKind = 'certificate' | 'local';

/**
 * Exchanges the `code` the sign-in page gave the user for a credential and
 * adds (or re-authenticates) the account it belongs to.
 */
export interface CompleteConnectorAuthenticationRequest {
	extension_id: number;
	code: string;
}

/**
 * An interface in CohesionUI, such as what a connector's
 * `renderResource(resource, kind)` returns to show a resource in place of
 * its raw contents: its blocks, top to bottom.
 *
 * ```js
 * function renderResource(resource, kind) {
 *   const row = JSON.parse(resource.contents);
 *   return {
 *     blocks: [
 *       { kind: "heading", level: 1, text: resource.title },
 *       { kind: "overview", title: "Status", child: { kind: "text", text: row.status } },
 *       { kind: "checkbox", checked: row.done, text: "Done" },
 *       { kind: "link", text: "Open in Notion", destination: row.url },
 *     ],
 *   };
 * }
 * ```
 */
export interface Component {
	/** What the interface shows, top to bottom. */
	blocks: Block[];
}

/** A connector's stored definition, which its accounts share. */
export interface Connector {
	/** Unique, stable identifier. */
	id: number;
	/** Identifies the connector to its hosts, unique ignoring case. */
	identifier: string;
	version: number;
	/** The service's name, such as `"Notion"`. */
	title: string;
	/** A `data:image/…` or `https://` URL. */
	icon: string | null;
	/** What the service calls an account, such as `"Workspace"` or `"Device"`. */
	unit: string;
	/**
	 * The kinds of item its resources can be, as its definition last
	 * declared them.
	 */
	kinds: ResourceKind[];
	/** The metadata keys its resources may carry. */
	metadata_keys: MetadataKey[];
	/** When the connector was first served, in Unix seconds. */
	created_at: number;
}

/** The sign-in page to open for the user. */
export interface ConnectorAuthentication {
	url: string;
}

/**
 * One service an `oauth` extension reaches, as its `serviceDescription()`
 * describes it.
 */
export interface ConnectorService {
	extension_id: number;
	/** The extension's name. */
	name: string;
	/** The service's name; the extension's name when `error` is set. */
	title: string;
	unit: string;
	/** The extension's icon, a `data:image/…` or `https://` URL. */
	icon: string | null;
	/** Why the extension couldn't describe its service. */
	error: string | null;
	/**
	 * What to do on the sign-in page, when the service doesn't end its
	 * sign-in by showing a code.
	 */
	sign_in_hint: string | null;
}

/** A person a workspace keeps: their name, the ways to reach them and notes. */
export interface Contact {
	/** Unique, stable identifier. */
	id: number;
	/** Id of the workspace the contact belongs to. */
	workspace_id: number;
	/**
	 * Whether the contact has been soft-deleted.
	 *
	 * Listings skip deleted contacts, so this is normally `false`.
	 */
	deleted: boolean;
	/** Never empty. */
	first_name: string;
	/** `null` when the contact has none; never empty. */
	middle_name: string | null;
	/** Never empty. */
	last_name: string;
	/** Free text about the contact, as plain text / Markdown. */
	notes: string;
	/** When the contact was added, in Unix seconds. */
	created_at: number;
	/**
	 * When its names, notes, methods or photo last changed, in Unix
	 * seconds.
	 */
	modified: number;
	/** Its photo, if it has one. */
	photo: Photo;
	/**
	 * The email addresses and phone numbers that reach the contact, in the
	 * order they were added.
	 */
	methods: ContactMethod[];
}

/** One way to reach a contact: an email address or a phone number. */
export interface ContactMethod {
	/** Unique, stable identifier. */
	id: number;
	/** Id of the workspace its contact belongs to. */
	workspace_id: number;
	/** Id of the contact it reaches. */
	contact_id: number;
	/** Whether `value` is an email address or a phone number. */
	kind: ContactMethodKind;
	/** The address or number, as written; never empty. */
	value: string;
	/** When the method was added, in Unix seconds. */
	created_at: number;
	/** When its kind or value last changed, in Unix seconds. */
	modified: number;
}

/**
 * What a contact method reaches; stored as text.
 *
 * - `"email"`: an email address.
 * - `"phone"`: a phone number.
 */
export type ContactMethodKind = 'email' | 'phone';

/**
 * Why an agent's turn started a fresh model thread instead of carrying on
 * its last one, in a `rebuiltContext` message.
 */
export type ContextRebuildReason =
	| 'cacheExpired'
	| 'contextLimit'
	| 'memoryCapacityChanged'
	| 'modelChanged'
	| 'noThread'
	| 'turnFailed';

/** How full a session's context window is, in tokens. */
export interface ContextUsage {
	/** The tokens the context holds now, never fewer than none. */
	used_tokens: number;
	/** The most it can hold, more than zero. */
	total_tokens: number;
}

/** Creates a team of agents, with none on it yet. */
export interface CreateAgentTeamRequest {
	/** Unique among teams, ignoring case. */
	title: string;
}

/**
 * Creates a contact in workspace `workspace_id`, with `methods` in their
 * order. The first and last names may not be empty, nor may a method's
 * value; an empty middle name is none.
 */
export interface CreateContactRequest {
	workspace_id: number;
	first_name: string;
	middle_name?: string | null;
	last_name: string;
	notes: string;
	methods: NewContactMethod[];
}

/** Creates a document folder in a workspace, last among its siblings. */
export interface CreateDocumentFolderRequest {
	workspace_id: number;
	title: string;
	/** The live folder to create it in; the top level when absent. */
	parent_id?: number | null;
}

/** Creates an ordinary document in a workspace. */
export interface CreateDocumentRequest {
	workspace_id: number;
	title: string;
	contents: string;
	/** The live document folder to file it in; the Inbox when absent. */
	folder_id?: number | null;
}

/** Creates an agent; it runs on the `agent` tier's default model. */
export interface CreateIntelligenceAgentRequest {
	/** A blank or absent name picks an unused first name, like `Ada`. */
	name?: string | null;
	color: IntelligenceAgentColor;
	/** Shown in place of the name's initial; none when absent. */
	emoji?: string | null;
	/** `balanced` when absent. */
	personality?: IntelligenceAgentPersonality | null;
	/** `brief` when absent. */
	verbosity?: IntelligenceAgentVerbosity | null;
	/** What the user wants from the agent; none when absent. */
	role?: string | null;
	/** The rules the agent always follows; none when absent. */
	directive?: string | null;
	/** The team it starts on; none when absent. */
	team_id?: number | null;
}

/**
 * Creates an intelligence endpoint from a preset, with the preset's default
 * model imported. Refused when the name is taken.
 */
export type CreateIntelligenceEndpointRequest = {
	/**
	 * What the endpoint is called. Defaults to the provider's display name,
	 * or a custom endpoint's provider identifier.
	 */
	name?: string | null;
	api_key?: string | null;
	/**
	 * The page where a person sees what the endpoint has used. Defaults to
	 * the provider's usage page; a custom endpoint has none.
	 */
	usage_url?: string | null;
	/**
	 * How many minutes the server keeps a prompt cached, at least 1.
	 * Defaults to the provider's; a custom endpoint's to 5.
	 */
	cache_minutes?: number | null;
	/**
	 * The longer cache an agent's requests ask for. Defaults to the
	 * provider's; a custom endpoint has none.
	 */
	long_cache?: LongCache | null;
} & IntelligenceEndpointPreset;

/**
 * Saves a way of looking at a workspace's tasks under a name, last among
 * its views.
 */
export interface CreateSavedTaskViewRequest {
	workspace_id: number;
	/** Unique, ignoring case, among the workspace's views. */
	name: string;
	scope: TaskViewScope;
	tags: TagFilter;
	/** Every priority when empty. */
	priorities: TaskPriority[];
	/** Every task, sized or not, when empty or absent. */
	sizes?: TaskSize[];
	sort: TaskSort;
	descending: boolean;
}

/**
 * Creates a tag in workspace `workspace_id`. A workspace has one tag per
 * name, ignoring case.
 */
export interface CreateTagRequest {
	workspace_id: number;
	name: string;
}

/** Create task folder */
export interface CreateTaskFolderRequest {
	workspace_id: number;
	title: string;
}

/** Create task */
export interface CreateTaskRequest {
	workspace_id: number;
	title: string;
	folder_id?: number | null;
	/** `"none"` when absent. */
	priority?: TaskPriority | null;
	/** None when absent. */
	size?: TaskSize | null;
	/** No one when absent. */
	assignee?: TaskAssignee | null;
	/** Markdown notes; none when absent. */
	description?: string | null;
	/** The start day, as `"YYYY-MM-DD"`; none when absent. */
	start_date?: string | null;
	/** The due day, as `"YYYY-MM-DD"`; none when absent. */
	due_date?: string | null;
	/** Ids of the workspace's task tags to put on the task; none when absent. */
	tag_ids?: number[];
}

/** Creates a workspace with `title`. */
export interface CreateWorkspaceRequest {
	title: string;
	/** Shown in place of the title's initial; none when absent. */
	emoji?: string | null;
}

/** A backup the daemon wrote. */
export interface DatabaseBackup {
	/** Where it is, on the daemon's computer. */
	path: string;
	/** How large it is, in bytes. */
	bytes: number;
}

/** An export the daemon wrote, waiting to be read. */
export interface DatabaseExport {
	id: string;
	/** How large it is, in bytes. */
	bytes: number;
}

/** Part of an export. */
export interface DatabaseExportPart {
	data: string;
}

/** An import under way. */
export interface DatabaseImport {
	id: string;
}

/** How large the database file is, and what fills it. */
export interface DatabaseSize {
	/** The whole file, in bytes: its pages, without the `-wal` file beside it. */
	bytes: number;
	/**
	 * What of it is free pages, in bytes: space SQLite reuses before the
	 * file grows, given back once the cleanup vacuums.
	 */
	free_bytes: number;
	/**
	 * What each area's tables and their indexes take up, largest first and
	 * [`StorageArea::Other`] last. With `free_bytes` they add up to `bytes`.
	 */
	areas: StorageAreaSize[];
}

/**
 * Which days a date filter matches, each day as `"YYYY-MM-DD"`:
 * - a day: only that day.
 * - an array of days: any of them.
 * - a `DayRange`: the days between its bounds.
 */
export type DayFilter = string | string[] | DayRange;

/**
 * The days strictly between two `"YYYY-MM-DD"` bounds; a bound left out leaves
 * that side open.
 *
 * A task with no date never matches.
 */
export interface DayRange {
	/** Only days before this one. */
	before?: string | null;
	/** Only days after this one. */
	after?: string | null;
}

/**
 * Deletes team `team_id`: its agents stay, on no team, and its memories
 * are forgotten.
 */
export interface DeleteAgentTeamRequest {
	team_id: number;
}

/** Revokes client `id`; its certificate is no longer trusted. */
export interface DeleteClientRequest {
	id: number;
}

/**
 * Removes connector account `account_id` and every resource imported from
 * it.
 */
export interface DeleteConnectorAccountRequest {
	account_id: number;
}

/**
 * Removes contact method `method_id` from a live contact in workspace
 * `workspace_id`.
 */
export interface DeleteContactMethodRequest {
	workspace_id: number;
	method_id: number;
}

/** Deletes contact `contact_id` in workspace `workspace_id`. */
export interface DeleteContactRequest {
	workspace_id: number;
	contact_id: number;
}

/**
 * Deletes document folder `folder_id` in a workspace and every folder
 * inside it; the documents they held move to the Inbox.
 */
export interface DeleteDocumentFolderRequest {
	workspace_id: number;
	folder_id: number;
}

/** Deletes document `document_id` in a workspace. */
export interface DeleteDocumentRequest {
	workspace_id: number;
	document_id: number;
}

/** Stops following feed `feed_id` and removes its items. */
export interface DeleteFeedRequest {
	feed_id: number;
}

/** Deletes agent `agent_id` with its cues, runs and messages. */
export interface DeleteIntelligenceAgentRequest {
	agent_id: number;
}

/** Deletes intelligence endpoint `id`. */
export interface DeleteIntelligenceEndpointRequest {
	id: number;
}

/** Removes resource `resource_id` from workspace `workspace_id`. */
export interface DeleteResourceRequest {
	workspace_id: number;
	resource_id: number;
}

/** Deletes saved view `view_id`; its tasks stay as they are. */
export interface DeleteSavedTaskViewRequest {
	workspace_id: number;
	view_id: number;
}

/**
 * Deletes tag `tag_id` of workspace `workspace_id`, taking it off every
 * task it was on.
 */
export interface DeleteTagRequest {
	workspace_id: number;
	tag_id: number;
}

/** Delete task folder */
export interface DeleteTaskFolderRequest {
	workspace_id: number;
	folder_id: number;
}

/** Deletes task `task_id` in a workspace. */
export interface DeleteTaskRequest {
	workspace_id: number;
	task_id: number;
}

/** Deletes workspace `id`. */
export interface DeleteWorkspaceRequest {
	id: number;
}

/**
 * A deleted item Rewind can still bring back: what its `"deleted"` version
 * holds.
 */
export interface DeletedItem {
	/** What was deleted. */
	item: VersionedItem;
	title: string;
	/** For a journal entry, the day it belongs to. */
	journal_date: string | null;
	/** Who deleted it. */
	author: VersionAuthor;
	author_name: string | null;
	/** When it was deleted, in Unix seconds. */
	modified: number;
}

/**
 * The wire dialect an intelligence endpoint speaks, which selects the backend
 * used to talk to it.
 */
export type Dialect = 'chatCompletions' | 'messages' | 'responses' | 'statelessResponses';

/** A piece of text in a workspace; journals are documents with a `journal_date`. */
export interface Document {
	/** Unique, stable identifier. */
	id: number;
	/** Id of the workspace this document belongs to. */
	workspace_id: number;
	/**
	 * Whether the document has been soft-deleted.
	 *
	 * Listings skip deleted documents, so this is normally `false`.
	 */
	deleted: boolean;
	/** Whether this is an ordinary document or a daily journal. */
	document_type: DocumentType;
	/**
	 * Display name.
	 *
	 * Journals are titled with their date, as `YYYY-MM-DD`.
	 */
	title: string;
	/** The full text of the document, as plain text / Markdown. */
	contents: string;
	/**
	 * When the contents last changed, in Unix **seconds** (not
	 * milliseconds; in JavaScript use `new Date(modified * 1000)`).
	 */
	modified: number;
	/**
	 * For journals, the day the entry belongs to, as `"YYYY-MM-DD"`.
	 *
	 * `null` for normal documents.
	 */
	journal_date: string | null;
	/**
	 * Id of the document folder this document is filed in, or `null` when
	 * it is in the Inbox.
	 *
	 * Always `null` for journals.
	 */
	folder_id: number | null;
}

/**
 * A named group of documents in a workspace, which may sit inside another.
 *
 * Folders nest without limit. A document in no folder is in the Inbox;
 * journals are never in a folder.
 */
export interface DocumentFolder {
	id: number;
	workspace_id: number;
	/** Id of the folder this one sits in, or `null` at the top level. */
	parent_id: number | null;
	title: string;
	/** Where the folder sits among its siblings; lower comes first. */
	sort_order: number;
	deleted: boolean;
}

/**
 * What a document is for; stored as text.
 *
 * - `"normal"`: an ordinary document.
 * - `"journal"`: a daily journal entry; one per workspace per local day.
 */
export type DocumentType = 'normal' | 'journal';

export interface DocumentationResponse {
	documentation: string;
}

/** Replaces the contents of live document `document_id` in a workspace. */
export interface EditDocumentRequest {
	workspace_id: number;
	document_id: number;
	contents: string;
}

/**
 * Replaces the contents of the journal for `date` (`"YYYY-MM-DD"`) in a
 * workspace, creating it if needed.
 */
export interface EditJournalRequest {
	workspace_id: number;
	date: string;
	content: string;
}

/** Replaces the description of task `task_id` in a workspace; `""` clears it. */
export interface EditTaskDescriptionRequest {
	workspace_id: number;
	task_id: number;
	description: string;
}

/**
 * A model an intelligence endpoint offers, one of the models every picker
 * lists. A row the models.dev catalog lists links its catalog row and reads
 * through that link whatever it does not set itself, so catalog prices
 * never drift; a value set on the row overrides the catalog's. Every read
 * joins the catalog, so the fields here are the model's either way. Prices
 * are USD per million tokens; a model without input and output prices
 * records no cost.
 */
export interface EndpointModel {
	/** Unique, stable identifier. */
	id: number;
	/** The endpoint that serves the model. */
	endpoint_id: number;
	/** The model's identifier on that endpoint, such as `claude-opus-5-5`. */
	model_identifier: string;
	/** The model's display name, such as `Claude Opus 5.5`. */
	model_name: string;
	/** The catalog row the model was imported from, if any. */
	catalog_model_id: number | null;
	/** How many tokens the model's context window holds. */
	context_size: number | null;
	input_token_cost: number | null;
	output_token_cost: number | null;
	cache_read_token_cost: number | null;
	cache_write_token_cost: number | null;
	/**
	 * Past this many prompt tokens a request is billed at the large-context
	 * prices.
	 */
	large_context_threshold: number | null;
	large_context_input_token_cost: number | null;
	large_context_output_token_cost: number | null;
	large_context_cache_read_token_cost: number | null;
	large_context_cache_write_token_cost: number | null;
}

/** The kind of failure, for clients that branch on it. */
export type ErrorCode =
	| 'badRequest'
	| 'validationFailed'
	| 'internalError'
	| 'upstreamError'
	| 'notFound'
	| 'unauthorized';

/** What an extension's `serviceRequest` returned; `null` for `undefined`. */
export interface ExtensionCallResult {
	result: unknown;
}

export interface ExtensionHook {
	id: number;
	extension_id: number;
	hook: HookKind;
	enabled: boolean;
}

/**
 * What an extension's global `manifest()` function returns. The daemon
 * reads it at install time, in a sandbox with no globals but entity classes
 * and enums.
 */
export interface ExtensionManifest {
	/**
	 * Stable, unique key for the extension, such as
	 * `"com.example.done-stamp"`. Installing a file with an identifier that
	 * already exists replaces that extension.
	 */
	identifier: string;
	/** Display name. */
	name: string;
	/**
	 * A whole number, raised with each release: a registry listing a
	 * higher one offers it as an update, and for an `oauth` extension it
	 * is also its connector definition's version.
	 */
	version: number;
	/** What the extension does, in a sentence or two. */
	description: string | null;
	/** Who wrote the extension. */
	author: string | null;
	/**
	 * The extension's icon: a `data:image/…` URL embedded in the script, or
	 * an `https://` URL. Aim for about 100×100 px; a PNG or a small SVG is
	 * plenty. At most 64 KB. Anything else is dropped without an error, and
	 * the extension's initial is shown instead.
	 */
	icon: string | null;
	/** What the extension may do. An unknown permission rejects the install. */
	permissions: ExtensionPermissionKind[];
	/**
	 * The events the daemon calls the extension for. An unknown hook
	 * rejects the install.
	 */
	hooks: HookKind[];
	/**
	 * The functions the extension serves at `serviceRequest`, for a
	 * manifest that lists `service` or `service:isolated`. Once declared,
	 * a call to any other name is refused.
	 */
	service: ServiceManifest | null;
}

export interface ExtensionPermission {
	id: number;
	extension_id: number;
	permission: ExtensionPermissionKind;
	enabled: boolean;
}

/**
 * A permission an extension's manifest lists. `cohesion:read` grants the
 * `cohesion` global read-only: any write throws. `cohesion:write` grants it
 * with writes, and makes the entities hooks receive writable too. `network`
 * grants the `network` global and `timer` the `timer` global. `registry`
 * grants no global: it lets the daemon call the extension's globals
 * `registryResults(query: string)`, which returns `RegistryResult[]`
 * (`{ manifest, sourceURL }`) listing extensions it offers, and
 * `registryLookup(identifiers: string[])`, which returns a `RegistryResult`
 * for each of those identifiers it offers and leaves the rest out; a
 * registry must define both. `oauth` lets the
 * daemon call the extension's sign-in functions (`serviceDescription`,
 * `authenticationURL`, `completeAuthentication`, `completeRefresh` and
 * `getAccountDescription`) so the user can add accounts at a service, and
 * the optional `accountRemoved(account: AccountDescription)` once one is
 * removed. It also grants the `accounts` global, which lists those
 * accounts with their credentials. `connector` lets the daemon call `getOrganizationUnits`,
 * `listResources` and `getResource` to import an account's items as resources; a
 * manifest that lists `connector` must list `oauth` too. `service` grants
 * no global: it makes the extension a service clients call, through the
 * `callExtension` command, at its global
 * `serviceRequest(name: string, args: unknown)`, receiving what it returns;
 * with a `service` block in the manifest, agents, scripts and extensions
 * granted `services` can call it too. `service:isolated` is `service` for
 * clients only: it never reaches `services`. A manifest lists one or the
 * other, never both. `services` grants the `services` global, through
 * which the extension calls other installed services. `usage` grants the
 * `usage` global: how many tokens Cohesion's models used and what they
 * cost. `intelligence` grants the `intelligence` global and lets the
 * extension change intelligence settings (endpoints and their models),
 * which `cohesion:write` does not. `notifications` grants the
 * `notifications` global, which sends the user a notification.
 */
export type ExtensionPermissionKind =
	| 'cohesion:read'
	| 'cohesion:write'
	| 'network'
	| 'timer'
	| 'registry'
	| 'oauth'
	| 'connector'
	| 'service'
	| 'service:isolated'
	| 'services'
	| 'usage'
	| 'intelligence'
	| 'notifications'
	| 'actors';

/**
 * An extension script as downloaded or given, for review before installing it with
 * `registerExtension`.
 */
export interface ExtensionPreview {
	script: string;
	/** The manifest the script itself declares. */
	manifest: ExtensionManifest;
	/**
	 * The installed extension with the same identifier, which installing
	 * this script replaces.
	 */
	installed: ExtensionSummary | null;
}

/** An installed extension without its script. */
export interface ExtensionSummary {
	id: number;
	name: string;
	version: number;
	script_identifier: string;
	description: string;
	author: string;
	/** A `data:image/…` or `https://` URL. */
	icon: string | null;
	/** Hook evaluation order: lower runs first, ties by `id`. */
	priority: number;
}

/**
 * A registry's entry for an installed extension at a higher version than
 * the installed one.
 */
export interface ExtensionUpdate {
	/** The installed extension's id. */
	extension_id: number;
	/** The id of the registry extension that offers it. */
	registry_extension_id: number;
	result: RegistryResult;
}

/**
 * An RSS or Atom feed, followed once for every workspace. Each refresh
 * adds the items it hasn't seen as articles, which make up the feed corpus;
 * nothing a refresh no longer sees is removed.
 */
export interface Feed {
	/** Unique, stable identifier. */
	id: number;
	/**
	 * Whether the feed has been deleted. Listings skip deleted feeds, so
	 * this is normally `false`.
	 */
	deleted: boolean;
	/** The feed's address; unique among the live feeds. */
	url: string;
	/** Display name: the feed's own title when it was added, until renamed. */
	title: string;
	/** Where the feed sits among the feeds; lower comes first. */
	sort_order: number;
	/** When an item's page is fetched rather than read from the feed. */
	full_page_policy: FeedFullPagePolicy;
	/** How the last refresh went. */
	status: FeedStatus;
	/** When the feed was added, in Unix seconds. */
	created_at: number;
}

/** What cleaning up feeds removed. */
export interface FeedCleanup {
	/** How many items were removed. */
	removed: number;
}

/**
 * An article from a feed matching a corpus search, with the passage that
 * matched. Read only by the corpus search, which joins each match as `hit`.
 */
export interface FeedCorpusHit {
	id: number;
	feed_id: number;
	read: boolean;
	url: string;
	title: string;
	author: string | null;
	/** In Unix seconds. */
	published_at: number;
	published_precision: PublishedPrecision;
	/** A few words of the article around the match, in Markdown. */
	excerpt: string;
}

/**
 * One page of a corpus search: its matches, and how many the whole search
 * has.
 */
export interface FeedCorpusPage {
	hits: FeedCorpusHit[];
	/** Every match, on any page. */
	total: number;
}

/**
 * When an item's text comes from its page rather than from the feed.
 *
 * - `"whenExcerpt"`: only when the feed carries an excerpt, not the whole
 *   article (the default).
 * - `"always"`: every item's page is fetched.
 */
export type FeedFullPagePolicy = 'whenExcerpt' | 'always';

/** How a feed was last refreshed. */
export type FeedStatus =
	| {
			state: 'refreshed';
			/** When it was read, in Unix seconds. */
			at: number;
			/** How many items were new. */
			added: number;
	  }
	| {
			state: 'failed';
			/** Why it couldn't. */
			failure: ArticleFetchFailure;
			/** When it was attempted, in Unix seconds. */
			at: number;
	  };

/** A feed as the feed list shows it, with how many of its items are unread. */
export interface FeedSummary {
	id: number;
	deleted: boolean;
	url: string;
	title: string;
	sort_order: number;
	full_page_policy: FeedFullPagePolicy;
	status: FeedStatus;
	/** In Unix seconds. */
	created_at: number;
	/** How many of its live items haven't been read. */
	unread: number;
}

/**
 * Replaces everything in the database with import `id`, then restarts the
 * daemon. The daemon keeps its key and the clients paired with it; the
 * clients the import holds are revoked.
 */
export interface FinishDatabaseImportRequest {
	id: string;
}

/** Forgets memory `memory_id`. */
export interface ForgetMemoryRequest {
	memory_id: number;
}

/** Fetches actor session `id`. */
export interface GetActorSessionRequest {
	id: number;
}

/** Fetches agent message `message_id`. */
export interface GetAgentMessageRequest {
	message_id: number;
}

/** Fetches live article `article_id`, whole. Fetching it doesn't mark it read. */
export interface GetArticleRequest {
	article_id: number;
}

/** Fetches live contact `contact_id` in workspace `workspace_id`. */
export interface GetContactRequest {
	workspace_id: number;
	contact_id: number;
}

/** Fetches live document `document_id` in a workspace. */
export interface GetDocumentRequest {
	workspace_id: number;
	document_id: number;
}

/** Fetches agent `agent_id`. */
export interface GetIntelligenceAgentRequest {
	agent_id: number;
}

/** Fetches intelligence thread `thread_id` with its messages. */
export interface GetIntelligenceThreadRequest {
	thread_id: number;
}

/**
 * Fetches the journal for `date` (`"YYYY-MM-DD"`) in a workspace, or `None`
 * if nothing has been written that day. Never creates one.
 */
export interface GetJournalRequest {
	workspace_id: number;
	date: string;
}

/** Fetches resource `resource_id` in workspace `workspace_id`. */
export interface GetResourceRequest {
	workspace_id: number;
	resource_id: number;
}

/** Fetches one kept version. */
export interface GetVersionRequest {
	workspace_id: number;
	version_id: number;
}

/** `kind: "heading"`: a title over what follows. */
export interface HeadingBlock {
	/** 1 is the largest, 3 the smallest. */
	level: HeadingLevel;
	/** The heading's text. */
	text: string;
}

/** How large a heading is: 1, 2 or 3, largest first. */
export type HeadingLevel = 1 | 2 | 3;

/**
 * An event the daemon calls scripts for: extensions that list it in their
 * manifest's `hooks`, and agents with a cue on it. `launch` calls
 * `onLaunch()`; `task:create` calls `onTaskCreate(task)`, `task:status`
 * `onTaskStatus(task)`, and so on for each `task:*`, `taskfolder:*` and
 * `workspace:*` kind with its entity (`task:tagged` when a tag is put on or
 * taken off a task, `task:delete` with the task as it was deleted), except
 * `task:start` and `task:due`, which call `onTaskStart(task)` and
 * `onTaskDue(task)` at `day:start` for each incomplete task whose start or
 * due date is that day;
 * `journal:settled` calls `onJournalSettled(document)` two minutes after a
 * journal's last save, and `document:settled` `onDocumentSettled(document)`
 * the same for any other document;
 * `resource:import` calls `onResourceImport(resource)` when a resource is imported,
 * `resource:update` calls `onResourceUpdate(resource)` when a refresh changes one
 * and `resource:delete` `onResourceDelete(resource)` with one as it was removed;
 * `feed:article` calls `onFeedArticle(article)` for each new article a feed
 * refresh brings in, `feed:delete` `onFeedDelete(feed)` with a feed as it was
 * unfollowed and `article:read` `onArticleRead(article)` when one goes from
 * unread to read;
 * `agent:message` calls `onAgentMessage(message)` when a message lands in an
 * agent's conversation and `agent:reaction` calls `onAgentReaction(message)`
 * when the user or the agent reacts to one; `activity:started` calls
 * `onActivityStarted(activity)` when an activity starts,
 * `activity:updated` `onActivityUpdated(activity)` when its progress,
 * status or title changes and `activity:ended` `onActivityEnded(activity)` when it
 * ends; `actor:moved` calls `onActorMoved(session)` when an actor session's
 * status changes; `agent:turn:start` calls
 * `onAgentTurnStart(agent)` when an agent starts answering the user and
 * `agent:turn:end` `onAgentTurnEnd(agent)` when it finishes; `day:start` calls `onDayStart()` at 6:00 and
 * `day:end` calls `onDayEnd()` at 23:00, and `timer` calls
 * `onTimer(timer)` at the start of every minute, in the daemon's local time;
 * `extension:update` calls `onExtensionUpdate(update)` once for each
 * version a registry offers an installed extension at, across restarts;
 * `notification:send` calls `onNotification(notification)` when a script or
 * an agent sends the user a notification, for an extension to push to
 * their devices; `client:revoke` calls `onClientRevoke(client)` when a
 * client is revoked, for an extension to forget what it keeps for it.
 */
export type HookKind =
	| 'launch'
	| 'task:create'
	| 'task:status'
	| 'task:rename'
	| 'task:move'
	| 'task:priority'
	| 'task:size'
	| 'task:assigned'
	| 'task:description'
	| 'task:startdate'
	| 'task:duedate'
	| 'task:start'
	| 'task:due'
	| 'task:tagged'
	| 'task:delete'
	| 'taskfolder:create'
	| 'taskfolder:delete'
	| 'workspace:create'
	| 'workspace:delete'
	| 'journal:settled'
	| 'document:settled'
	| 'resource:import'
	| 'resource:update'
	| 'resource:delete'
	| 'feed:article'
	| 'feed:delete'
	| 'article:read'
	| 'agent:message'
	| 'agent:reaction'
	| 'agent:turn:start'
	| 'agent:turn:end'
	| 'activity:started'
	| 'activity:updated'
	| 'activity:ended'
	| 'actor:moved'
	| 'day:start'
	| 'day:end'
	| 'timer'
	| 'extension:update'
	| 'notification:send'
	| 'client:revoke';

/** When a hook fires, as clauses that finish "When …". */
export interface HookWording {
	kind: HookKind;
	/** As an extension's hook. */
	extension_clause: string;
	/** As an agent's cue or run; an agent never hears its own conversation. */
	cue_clause: string;
}

/**
 * An image to attach to a message to an agent or in an actor session: PNG,
 * JPEG, GIF or WebP, at most 5 MB once base64-encoded.
 */
export interface ImageUpload {
	/** `image/png`, `image/jpeg`, `image/gif` or `image/webp`. */
	media_type: string;
	/** The image, base64-encoded. */
	data: string;
}

/**
 * Imports item `identifier` of connector account `account_id` into
 * workspace `workspace_id`, or refreshes it if it is already there.
 */
export interface ImportResourceRequest {
	account_id: number;
	workspace_id: number;
	identifier: string;
}

/** One item a connector account offers for import. */
export interface ImportableResource {
	identifier: string;
	title: string;
	/**
	 * When the item last changed, in Unix seconds; absent when the service
	 * doesn't say.
	 */
	last_updated: number | null;
	/** Whether the item is already a resource in the workspace. */
	imported: boolean;
}

/**
 * Who started a chain of an agent's work, which every turn of the chain
 * inherits and which alone decides what it may change.
 *
 * - `"user"`: the user's message or pick, and every question and actor
 *   follow-up that descends from it: everything the conversation may do.
 * - `"event"`: one of the agent's cues: it reads, and asks the user before
 *   changing anything.
 */
export type Initiative = 'user' | 'event';

/**
 * A persistent assistant that keeps its context over time and can wake
 * itself on a hook.
 */
export interface IntelligenceAgent {
	id: number;
	/** When the agent was created, in Unix seconds. */
	created_at: number;
	/** The agent's name, chosen by the user. */
	agent_name: string;
	/** The color the agent is drawn in. */
	agent_color: IntelligenceAgentColor;
	/** An emoji shown in place of the name's initial; none when unset. */
	agent_emoji: string | null;
	/** The manner the agent speaks in. */
	agent_personality: IntelligenceAgentPersonality;
	/** How much the agent says. */
	agent_verbosity: IntelligenceAgentVerbosity;
	/** What the user wants from the agent, in a few words; `""` when unset. */
	agent_role: string;
	/** The rules the user gave the agent to always follow; `""` when unset. */
	agent_directive: string;
	/**
	 * What the agent is doing while it works, such as `"Thinking"` or the
	 * title of the script it is running; `null` when it is not working.
	 */
	activity: string | null;
	/**
	 * When the running turn of the agent's conversation started, in Unix
	 * seconds; the user can stop it. `null` when none is running; forks
	 * don't count.
	 */
	foreground_started: number | null;
	/**
	 * How many tokens the conversation's live model thread held when its
	 * latest request was sent, as the provider counted them (cached or not):
	 * the size of the working set the next request builds on. Updated after
	 * each request of the conversation's turns; forks, jobs and cues don't
	 * count. `null` while the agent has no live thread, and for a new thread until its first
	 * request is answered.
	 */
	context_tokens: number | null;
	/**
	 * Whether the agent has posted since the user last read its
	 * conversation; questions between agents don't count.
	 */
	agent_unread: boolean;
	/**
	 * When the latest message in the agent's conversation was sent, in Unix
	 * seconds, not counting code runs, documentation lookups, memory
	 * changes, questions between agents or the agent's bookkeeping (notes,
	 * wake lines, compressed contexts); `null` before the first.
	 */
	last_message_at: number | null;
	/** Whether the agent is pinned, ahead of the others. */
	pinned: boolean;
	/**
	 * Where the agent sits among the pinned agents; lower comes first.
	 *
	 * Unpinned agents go by their latest message instead.
	 */
	sort_order: number;
	/**
	 * The team the agent is on; `null` when it's on none. Pinning is
	 * apart from teams: a pinned agent stays on its team.
	 */
	team_id: number | null;
	/** How many actor sessions not yet resolved the agent follows. */
	following_count: number;
}

/** The color an agent is drawn in. */
export type IntelligenceAgentColor =
	'blue' | 'green' | 'orange' | 'purple' | 'red' | 'pink' | 'teal' | 'yellow';

/**
 * A hook an agent watches to wake itself in the background.
 *
 * An agent can have several, each with its own name.
 */
export interface IntelligenceAgentCue {
	id: number;
	agent_id: number;
	/**
	 * What the agent calls the cue, unique among its cues; setting a cue
	 * with the same name replaces it.
	 */
	name: string;
	/** The hook that runs the cue. */
	hook: HookKind;
	/**
	 * A function expression called with the hook's subject; a truthy return
	 * wakes the agent.
	 */
	script: string;
	/** What the agent told itself to do when it wakes. */
	instructions: string;
	/** How capable a model each background run the cue wakes gets. */
	effort: IntelligenceEffort;
	/** When the cue was last set, in Unix seconds. */
	set_at: number;
}

/** One entry in an agent's conversation. */
export interface IntelligenceAgentMessage {
	id: number;
	agent_id: number;
	/** The background run that sent it, if any. */
	run_id: number | null;
	/**
	 * What woke the background run that sent this, e.g. `"Test task" was
	 * created`; `null` for foreground messages.
	 */
	run_description: string | null;
	/**
	 * What the message is to the fork `run_id` names; `null` for a message
	 * of the conversation itself.
	 */
	run_part: AgentRunPart | null;
	sent_at: number;
	sender: IntelligenceAgentMessageSender;
	/** What it says, stored as `kind` and the columns each kind sets. */
	message_content: AgentMessageContent;
	/**
	 * An emoji reacting to the message, from the side that didn't send it:
	 * the user on the agent's messages, the agent on the user's.
	 */
	reaction: string | null;
	/** When `reaction` was last set or cleared. */
	reacted_at: number | null;
	/**
	 * How many images came with it, the user's or the agent's, shown with
	 * its text; `ListAgentMessageAttachments` fetches them.
	 */
	attachment_count: number;
	/**
	 * The message of the same conversation this one replies to, which a
	 * client quotes above it; `null` when it stands alone.
	 */
	reply_to: number | null;
	/**
	 * The message in one line: what a reply quoting it shows, and how a
	 * client shows a kind it has no view for. Its text, a forward's note (else the item's title), a choice's
	 * question, a script's title, `Learning about network`, `Asked Scout:
	 * …`, `Scout asked: …`, or an error; for the agent's bookkeeping, what
	 * follows `excerpt_verb`: the memory, the note, or what woke it
	 * (`Actor session 264 stopped`, `Cue “Morning” fired`, `“Fix
	 * the build” was assigned to you`). A compressed context's is
	 * `Context rebuilt`, then why when it is known, with no verb.
	 */
	excerpt: string;
	/**
	 * What the agent did, for a line of its bookkeeping: `Remembered`,
	 * `Forgot`, `Noted` or `Woke`, which reads alone as the heading of a
	 * run of such lines and before `excerpt` as `Noted: …`; `null` for
	 * every other kind.
	 */
	excerpt_verb: string | null;
	/**
	 * What follows `reaction_excerpt_verb` in a line about `reaction`: the
	 * emoji and the message it reacts to, quoted and cut to 140 characters
	 * (`👍 to “Done.”`); `null` with no reaction.
	 */
	reaction_excerpt: string | null;
	/**
	 * `Reacted` when the message has a `reaction`, the heading of a run of
	 * lines about reactions and the start of one; `null` otherwise.
	 */
	reaction_excerpt_verb: string | null;
}

/**
 * Who sent an agent message.
 *
 * - `"user"`: the user.
 * - `"agent"`: the agent speaking to the user: in its conversation, or
 *   from a fork of it that another agent's question woke (the message's
 *   `run_id` names the fork's run).
 * - `"backgroundAgent"`: the agent in a run one of its cues woke, before
 *   cues woke forks; only its notes landed here. Another
 *   agent's question to it stands here too, opening the fork it woke.
 */
export type IntelligenceAgentMessageSender = 'user' | 'agent' | 'backgroundAgent';

/**
 * The manner an agent speaks in.
 *
 * - `"balanced"`: plain and even, like a helpful co-worker.
 * - `"friendly"`: warm and encouraging.
 * - `"professional"`: formal and neutral.
 * - `"candid"`: blunt; says when something looks wrong and pushes back.
 * - `"playful"`: light and humorous.
 */
export type IntelligenceAgentPersonality =
	'balanced' | 'friendly' | 'professional' | 'candid' | 'playful';

/**
 * One fork of an agent's conversation: one of its cues, another agent's
 * question or an actor session it follows woke it.
 */
export interface IntelligenceAgentRun {
	id: number;
	agent_id: number;
	/** The intelligence thread of the session, once it has one. */
	thread_id: number | null;
	/**
	 * The hook that fired; `agent:message` for a fork another agent's
	 * question woke, `actor:moved` for one an actor session it follows woke,
	 * `task:assigned` for one a task assigned to it woke.
	 */
	hook: HookKind;
	/**
	 * The name of the cue that started the run; `null` for a fork, and for
	 * runs from before agents had several cues.
	 */
	cue_name: string | null;
	/**
	 * The agent whose question woke the run, a fork of the conversation;
	 * `null` for a cue's run, or once that agent is deleted.
	 */
	sender_agent_id: number | null;
	started_at: number;
	/** Where it stands, with when it ended and why it failed. */
	state: IntelligenceAgentRunState;
	/**
	 * Prompt tokens across the run that were neither read from nor written
	 * to the cache.
	 */
	input_tokens: number;
	output_tokens: number;
	cache_read_tokens: number;
	cache_write_tokens: number;
	/** What the run cost; `null` when nothing was priced. */
	cost: number | null;
}

/**
 * Where a fork of an agent's conversation stands, tagged by `status`.
 *
 * - `"running"`: still going.
 * - `"completed"`: finished normally, at `ended_at`, in Unix seconds.
 * - `"failed"`: stopped on `error`, at `ended_at`.
 */
export type IntelligenceAgentRunState =
	| { status: 'running' }
	| {
			status: 'completed';
			ended_at: number;
	  }
	| {
			status: 'failed';
			ended_at: number;
			error: string;
	  };

/**
 * A team of agents: a named group whose members see each other in their
 * prompts and share the team's memories.
 */
export interface IntelligenceAgentTeam {
	id: number;
	/** The team's name, unique ignoring case. */
	title: string;
}

/**
 * How much an agent says, from least to most.
 *
 * - `"terse"`: as few words as will do.
 * - `"brief"`: short messages, detail only when asked.
 * - `"detailed"`: explains its reasoning and what it found.
 * - `"thorough"`: full context, options and caveats.
 */
export type IntelligenceAgentVerbosity = 'terse' | 'brief' | 'detailed' | 'thorough';

/** A record of what one model invocation cost. */
export interface IntelligenceCostEvent {
	id: number;
	created_at: number;
	/**
	 * The thread the invocation answered; `None` for events recorded before
	 * threads were linked.
	 */
	thread_id: number | null;
	provider_identifier: string;
	model_identifier: string;
	input_tokens: number;
	output_tokens: number;
	cache_read_tokens: number;
	cache_write_tokens: number;
	/** What the invocation cost in all, in USD. */
	total_cost: number;
	/**
	 * What each kind of token cost of the total; `None` when the provider
	 * reported only the total.
	 */
	input_cost: number | null;
	output_cost: number | null;
	cache_read_cost: number | null;
	cache_write_cost: number | null;
}

export interface IntelligenceDefault {
	id: number;
	/** The tier of the intelligence. */
	tier: IntelligenceTier;
	/** The endpoint model the tier uses. Removing that model unsets the tier. */
	endpoint_model_id: number;
}

/**
 * How much model a background run gets; each picks the matching tier's
 * default model.
 *
 * - `"low"`: quick checks and short replies.
 * - `"medium"`: ordinary work.
 * - `"high"`: hard, multi-step reasoning.
 */
export type IntelligenceEffort = 'low' | 'medium' | 'high';

/**
 * A known provider to create an endpoint for, or a custom endpoint
 * described by hand. On the wire this is `{"preset": "openAi"}` or
 * `{"preset": "custom", "provider_identifier": …, "dialect": …,
 * "base_url": …}`.
 */
export type IntelligenceEndpointPreset =
	| {
			preset: 'custom';
			provider_identifier: string;
			dialect: Dialect;
			base_url: string;
	  }
	| {
			preset: IntelligenceProvider;
	  };

/**
 * A provider with a built-in endpoint preset. Adding one takes a variant
 * here and its arm in [`IntelligenceProvider::defaults`]; the wire format
 * and the CLI pick it up from there.
 *
 * JSON uses the camelCase name (`openAi`); the CLI uses the lowercase text
 * (`openai`).
 */
export type IntelligenceProvider =
	| 'openAi'
	| 'anthropic'
	| 'xai'
	| 'mistral'
	| 'fireworks'
	| 'deepInfra'
	| 'togetherAI'
	| 'openRouter';

/**
 * What every model call cost over one period, against that period's
 * limit, if it has one.
 */
export interface IntelligenceSpend {
	period: SpendPeriod;
	/** In US dollars; `None` when the period has no limit. */
	limit: number | null;
	/** What the calls in the period cost, in US dollars. */
	spent: number;
	/**
	 * Whether `spent` has reached `limit`, which stops background work
	 * making model calls.
	 */
	reached: boolean;
}

/**
 * The messages sent to a model and what it answered, for an agent's
 * conversation or a fork of it.
 */
export interface IntelligenceThread {
	id: number;
	created_at: number;
	prompt_id: number;
	/** The system prompt, from `prompt_id`. */
	system_prompt: string;
	/** Whether it started empty or carries on another thread's messages. */
	origin: ThreadOrigin;
	/** The provider's own copy of the conversation, when it keeps one. */
	remote: RemoteChain | null;
}

/** An intelligence thread and its messages, oldest first. */
export interface IntelligenceThreadDetail {
	thread: IntelligenceThread;
	messages: IntelligenceThreadMessage[];
	/** What each model call in the thread cost, oldest first. */
	cost_events: IntelligenceCostEvent[];
}

/**
 * One message of a thread. A fork's own messages come after those it
 * inherits, and ids order them all.
 */
export interface IntelligenceThreadMessage {
	id: number;
	thread_id: number;
	created_at: number;
	/** What it says. */
	item: ThreadItem;
}

/**
 * Which default model a session uses.
 *
 * - `"agent"`: every agent conversation, and scripts that name no tier.
 * - `"low"`, `"medium"`, `"high"`: background runs, by the effort their
 *   cue asked for.
 */
export type IntelligenceTier = 'agent' | 'low' | 'medium' | 'high';

export type Invalidation =
	| { kind: 'workspace'; id: number }
	| { kind: 'intelligenceCostEvents' }
	| { kind: 'agent'; id: number }
	| { kind: 'agentTeams' }
	| { kind: 'taskFolders'; id: number }
	| { kind: 'documents'; id: number }
	| { kind: 'documentFolders'; id: number }
	| {
			kind: 'taskList';
			id: {
				workspace_id: number;
				folder_id: number | null;
			};
	  }
	| { kind: 'connectorAccounts' }
	| { kind: 'resources'; id: number }
	| { kind: 'resourceKinds'; id: number }
	| { kind: 'feeds' }
	| { kind: 'feedArticles' }
	| { kind: 'intelligence' }
	| { kind: 'clients' }
	| { kind: 'security' }
	| { kind: 'storage' }
	| { kind: 'profile' }
	| { kind: 'tags'; id: number }
	| { kind: 'contacts'; id: number }
	| { kind: 'savedTaskViews'; id: number }
	| { kind: 'extensionUpdates' }
	| { kind: 'memories' }
	| { kind: 'activities' }
	| { kind: 'actors' }
	| { kind: 'actorSessions' }
	| { kind: 'actorSession'; id: number }
	| { kind: 'actorUsage' }
	| {
			kind: 'notification';
			id: {
				title: string;
				message: string;
			};
	  };

export interface InvalidationBatch {
	keys: Invalidation[];
}

/** `kind: "link"`: text that opens `destination`. */
export interface LinkBlock {
	/** What the link says. */
	text: string;
	/**
	 * Where it goes: an `https://`, `http://` or `mailto:` URL. Any other
	 * address makes the whole interface invalid.
	 */
	destination: string;
}

/**
 * Lists the activities still running or waiting, then the `limit` that
 * ended most recently, newest first.
 */
export interface ListActivitiesRequest {
	limit: number;
}

/**
 * Lists actor session `session_id`'s family: the session Cohesion started
 * that it is, or runs under at any depth, then every subagent beneath that,
 * each after the one it runs under, first launched first.
 */
export interface ListActorSessionFamilyRequest {
	session_id: number;
}

/**
 * Lists the images attached to actor session message `message_id`, in the
 * order sent.
 */
export interface ListActorSessionMessageAttachmentsRequest {
	message_id: number;
}

/** Lists what was said in actor session `session_id`, oldest first. */
export interface ListActorSessionMessagesRequest {
	session_id: number;
}

/**
 * Lists actor sessions, latest status change first: actor `actor_id`'s, or
 * every actor's.
 */
export interface ListActorSessionsRequest {
	actor_id?: number | null;
}

/**
 * Lists the subagents actor session `session_id`'s actor launched from it,
 * one level down, first launched first. A subagent's own are listed by its
 * id.
 */
export interface ListActorSubagentsRequest {
	session_id: number;
}

/** Lists agent `agent_id`'s cues, oldest first. */
export interface ListAgentCuesRequest {
	agent_id: number;
}

/** Lists the images attached to message `message_id`, in the order sent. */
export interface ListAgentMessageAttachmentsRequest {
	message_id: number;
}

/**
 * Pages through an agent's messages: the `limit` newest with an id below
 * `before` (the newest overall when absent), oldest first.
 */
export interface ListAgentMessagesRequest {
	agent_id: number;
	before?: number | null;
	limit: number;
}

/**
 * Lists agent `agent_id`'s `limit` most recent background runs, newest
 * first.
 */
export interface ListAgentRunsRequest {
	agent_id: number;
	limit: number;
}

/**
 * Lists one page of the live articles `filter` covers, most recently
 * published first, without their contents.
 */
export interface ListArticlesRequest {
	filter: ArticleFilter;
	/** How many of the covered articles to skip; none when absent. */
	offset?: number;
	/** The most articles to list: 50 when absent, and never over 500. */
	limit?: number | null;
}

/**
 * Lists the catalog's models of provider `provider_identifier`, or of the
 * providers of configured endpoints when it is absent, whose identifier or
 * name contains `query`, if given.
 */
export interface ListCatalogModelsRequest {
	provider_identifier?: string | null;
	query?: string | null;
}

/** Lists the organization units of connector account `account_id`. */
export interface ListConnectorOrganizationUnitsRequest {
	account_id: number;
}

/**
 * Lists the live contacts in workspace `workspace_id`, by last name, then
 * first name, ignoring case.
 */
export interface ListContactsRequest {
	workspace_id: number;
}

/**
 * Lists the workspace's deleted items whose deletion Rewind still has, most
 * recently deleted first.
 */
export interface ListDeletedItemsRequest {
	workspace_id: number;
}

/**
 * Lists a workspace's live document folders, each with its `parent_id`,
 * in their sort order among their siblings.
 */
export interface ListDocumentFoldersRequest {
	workspace_id: number;
}

/** Lists the live documents in a workspace, most recently modified first. */
export interface ListDocumentsRequest {
	workspace_id: number;
	/** Only documents of this type; every type when absent. */
	document_type?: DocumentType | null;
	/** Only journals dated on or after this `"YYYY-MM-DD"`. */
	journal_from?: string | null;
	/** Only journals dated before this `"YYYY-MM-DD"`. */
	journal_to?: string | null;
}

/** Lists the models endpoint `endpoint_id` offers, by name. */
export interface ListEndpointModelsRequest {
	endpoint_id: number;
}

/** Lists the hooks recorded for extension `extension_id`. */
export interface ListExtensionHooksRequest {
	extension_id: number;
}

/** Lists the permissions recorded for extension `extension_id`. */
export interface ListExtensionPermissionsRequest {
	extension_id: number;
}

/** Lists the `limit` most recent intelligence cost events, newest first. */
export interface ListIntelligenceCostEventsRequest {
	limit: number;
}

/**
 * Lists the memories agent `agent_id` sees: its own and the global ones;
 * only the global ones when absent. Oldest first.
 */
export interface ListMemoriesRequest {
	agent_id?: number | null;
}

/**
 * Lists the resources in workspace `workspace_id` in `sort`'s order,
 * optionally only those of `kind` or whose title contains `search`.
 */
export interface ListResourcesRequest {
	workspace_id: number;
	kind?: string | null;
	search?: string | null;
	sort: ResourceSort;
	/** Reverses `sort`; resources without a value for it still come last. */
	descending: boolean;
}

/** Lists a workspace's saved task views, in their order. */
export interface ListSavedTaskViewsRequest {
	workspace_id: number;
}

/** One page of the security activity, newest first. */
export interface ListSecurityEventsRequest {
	offset?: number;
	limit?: number | null;
}

/** Lists workspace `workspace_id`'s tags, in their order. */
export interface ListTagsRequest {
	workspace_id: number;
}

/** Lists task folders */
export interface ListTaskFoldersRequest {
	workspace_id: number;
}

/** Lists one page of the live tasks the filters cover, in `sort`'s order. */
export interface ListTasksRequest {
	ids?: number[] | null;
	/** Only tasks in this workspace; every workspace when absent. */
	workspace_id?: number | null;
	/** Every task when absent. */
	folder?: TaskFolderFilter;
	statuses?: TaskStatus[] | null;
	/** Only tasks with these priorities. */
	priorities?: TaskPriority[] | null;
	/** Only tasks with these sizes; a task with no size never matches. */
	sizes?: TaskSize[] | null;
	/** Only tasks whose start day matches. */
	start_date?: DayFilter | null;
	/** Only tasks whose due day matches. */
	due_date?: DayFilter | null;
	/** Only tasks assigned to this assignee. */
	assignee?: TaskAssignee | null;
	/** Only tasks with every tag in `with` and none in `without`. */
	tags?: TagFilter;
	/** Creation order when absent. */
	sort?: TaskSort;
	/** Reverses `sort`; tasks without a value for it still come last. */
	descending?: boolean;
	/** How many of the covered tasks to skip; none when absent. */
	offset?: number;
	/** The most tasks to list: 50 when absent, and never over 500. */
	limit?: number | null;
}

/**
 * Lists the requests waiting on the user in actor session `session_id`'s
 * subagents, at any depth, oldest first: each a question, permission or
 * plan no one has responded to, of a subagent still going. A response goes
 * to the subagent's own session id, as for any request.
 */
export interface ListUnansweredSubagentRequestsRequest {
	session_id: number;
}

/**
 * Lists an item's kept versions, newest first, without their text. A deleted
 * item still has its history.
 */
export interface ListVersionsRequest {
	workspace_id: number;
	item: VersionTarget;
}

/** Lists the kinds of resource shown in workspace `workspace_id`. */
export interface ListWorkspaceResourceKindsRequest {
	workspace_id: number;
}

/**
 * How much of its limits one pool has used: the latest report among
 * actors `actor_ids`, which draw on it, least id first. `pool` is left out
 * for a single actor whose limits are its own.
 */
export type ListedActorUsage = {
	actor_ids: number[];
} & ActorUsage;

/**
 * A client as the clients list shows it. Presence is said here and only
 * here: a client's actors are reachable while the client is online.
 */
export type ListedClient = {
	/** Whether the client has a session open with the daemon now. */
	online: boolean;
	/** Whether it is the client that asked for the list. */
	current: boolean;
} & Client;

/**
 * A prompt cache longer than an endpoint's default, which a request asks
 * for with [`CacheLength::Long`](super::CacheLength::Long). Stored as JSON
 * text.
 */
export interface LongCache {
	/**
	 * How many minutes it lasts, which a request asks for as its `ttl`
	 * (`60` as `1h`, `30` as `30m`). At least 1.
	 */
	minutes: number;
	/**
	 * What a write to it costs, as a multiple of the model's write rate
	 * for the short cache, which is the only one the catalog (models.dev)
	 * publishes. More than 0.
	 */
	write_multiple: number;
}

/**
 * Marks actor session `session_id` read, now; the next stretch of work its actor
 * finishes marks it unread again.
 */
export interface MarkActorSessionReadRequest {
	session_id: number;
}

/** Marks agent `agent_id`'s conversation read. */
export interface MarkAgentReadRequest {
	agent_id: number;
}

/**
 * Marks every unread live article `filter` covers as read: "mark all as
 * read" for a view.
 */
export interface MarkArticlesReadRequest {
	filter: ArticleFilter;
}

/** A fact an agent keeps across conversations, in its own words. */
export interface Memory {
	id: number;
	/** Who sees it: one agent, one team's agents, or every agent. */
	scope: MemoryScope;
	created_at: number;
	memory: string;
}

/**
 * Who a memory is for. The table's check gives every row at most one of
 * an agent and a team.
 */
export type MemoryScope =
	| { kind: 'global' }
	| {
			kind: 'team';
			/** The team it belongs to. */
			team_id: number;
	  }
	| {
			kind: 'agent';
			/** The agent it belongs to. */
			agent_id: number;
	  };

/** What a diagram is drawn as. */
export type MermaidFormat = 'svg';

/** Why a diagram wasn't drawn. A client shows the source as code either way. */
export type MermaidRefusal =
	'unsupported' | 'syntax' | 'semantic' | 'tooLarge' | 'tooSlow' | 'busy' | 'failed';

/**
 * What a resource's metadata may say, as a connector fills it. Dates are
 * ISO 8601 in UTC with a `Z` suffix (`"2026-10-24T09:30:00Z"`), so they
 * sort as text; everything else is text as the service shows it.
 *
 * - `"dateCreated"`: when the item was created.
 * - `"dateModified"`: when the item's contents last changed.
 * - `"dateDue"`: when the item is due, for a task.
 * - `"dateStart"`: when an event starts.
 * - `"dateEnd"`: when an event ends.
 * - `"author"`: who wrote or owns the item.
 * - `"url"`: where the item opens at the service.
 * - `"location"`: where an event takes place, as the service shows it.
 * - `"folder"`: the folder, notebook or path holding the item, such as
 *   `"Work/Plans"`.
 * - `"category"`: the category, project or list the item is filed under.
 * - `"keywords"`: tags or labels, separated by commas.
 * - `"status"`: the item's state at the service, such as `"Done"`.
 */
export type MetadataKey =
	| 'dateCreated'
	| 'dateModified'
	| 'dateDue'
	| 'dateStart'
	| 'dateEnd'
	| 'author'
	| 'url'
	| 'location'
	| 'folder'
	| 'category'
	| 'keywords'
	| 'status';

/**
 * Moves every resource of connector account `from_account_id` to account
 * `to_account_id` of the same connector, in every workspace. Where both
 * hold the same item, the target's copy stays.
 */
export interface MigrateResourcesRequest {
	from_account_id: number;
	to_account_id: number;
}

/**
 * What a model costs, overriding the catalog, in USD per million tokens.
 * Cache tokens without their own price are billed as input.
 */
export interface ModelPrices {
	input_token_cost: number;
	output_token_cost: number;
	cache_read_token_cost?: number | null;
	cache_write_token_cost?: number | null;
}

/**
 * Moves document folder `folder_id` into folder `parent_id`, or the top
 * level when absent, just before its sibling `before_id`, or last when
 * absent. A folder can't move into itself or a folder inside it.
 */
export interface MoveDocumentFolderRequest {
	workspace_id: number;
	folder_id: number;
	parent_id?: number | null;
	before_id?: number | null;
}

/**
 * Files ordinary document `document_id` in a workspace into folder
 * `folder_id`, or the Inbox when absent.
 */
export interface MoveDocumentRequest {
	workspace_id: number;
	document_id: number;
	folder_id?: number | null;
}

/** Moves task `task_id` into folder `folder_id`, or the inbox when absent. */
export interface MoveTaskRequest {
	workspace_id: number;
	task_id: number;
	folder_id?: number | null;
}

/** A contact method to add with a new contact. */
export interface NewContactMethod {
	/** Whether `value` is an email address or a phone number. */
	kind: ContactMethodKind;
	/** The address or number, as written; may not be empty. */
	value: string;
}

/**
 * A part of an account that resources are grouped into, such as a team space
 * or a project, as `getOrganizationUnits(credential)` returns them.
 */
export interface OrganizationUnit {
	/** Identifies the unit to `listResources` as `organizationUnitId`. */
	id: string;
	/** The unit's name, shown to people. */
	display: string;
}

/**
 * `kind: "overview"`: one labelled value, such as a status or an
 * assignee, as the details at the top of a task show them. Overviews in a
 * row line up as one list.
 */
export interface OverviewBlock {
	/** The label, such as `"Status"`. */
	title: string;
	/** The value: usually a `text` block, or a `checkbox` or `link`. */
	child: Block;
}

/** Whether the client asking may open pairing now. */
export interface PairingAccess {
	/** Why it can't, as `BeginPairing` would refuse it; `None` when it can. */
	refusal: string | null;
}

/** The setup code a new client pairs with. */
export interface PairingCode {
	/**
	 * A [`SetupCode`](crate::SetupCode), encoded: it works for one
	 * registration, for ten minutes.
	 */
	code: string;
}

/**
 * Which clients may open pairing.
 *
 * - `"anyClient"`: any paired client.
 * - `"localClients"`: only clients on the same computer as Cohesion.
 */
export type PairingOpener = 'anyClient' | 'localClients';

/**
 * A photo of someone, a contact or the user, or that there is none;
 * stored as two nullable columns set together.
 *
 * - `"none"`: no photo.
 * - `"image"`: an image of them.
 */
export type Photo =
	| { kind: 'none' }
	| {
			kind: 'image';
			/** `image/png`, `image/jpeg`, `image/gif` or `image/webp`. */
			photo_media_type: string;
			/** The image, base64-encoded. */
			photo_data: string;
	  };

/** One limit of the plan an actor runs on, such as its five-hour limit. */
export interface PlanUsage {
	/** What the limit is, in a few words: "5-hour limit". */
	label: string;
	/** How much of it is used, from 0 to 1. */
	used: number;
	/** When it resets, in Unix seconds. Left out, unknown. */
	resets_at?: number | null;
}

/**
 * Downloads the extension script at `source_url` (`http` or `https`) and
 * reads its manifest, without installing it.
 */
export interface PreviewExtensionRequest {
	source_url: string;
}

/**
 * Reads the manifest of an extension `script` the client already has, such
 * as one from a file, without installing it.
 */
export interface PreviewExtensionScriptRequest {
	script: string;
}

/** The user's profile; there is always exactly one row. */
export interface Profile {
	id: number;
	/** Their name, `null` until they set one; never empty. */
	name: string | null;
	/**
	 * What they wrote about themselves, which every agent reads, `null` until
	 * they write some; never empty, and at most 1,000 characters.
	 */
	bio: string | null;
	/** Their photo, if they have set one. */
	photo: Photo;
}

/** A provider preset, and the catalog provider it reads models from. */
export interface ProviderWording {
	kind: IntelligenceProvider;
	label: string;
	provider_identifier: string;
}

/**
 * Removes the items of feeds `feed_ids` published before `before` (Unix
 * seconds). A removed item isn't added again when its feed is next read.
 */
export interface PruneFeedsRequest {
	feed_ids: number[];
	before: number;
}

/**
 * How precisely an article's publication is known.
 *
 * - `"time"`: the moment it was published, or brought in when neither its
 *   feed nor its page gives a date.
 * - `"day"`: only the day: `publishedAt` is that day's midnight UTC, to be
 *   shown as that date in every time zone.
 */
export type PublishedPrecision = 'time' | 'day';

/**
 * Answers the part of export `id` that starts at `offset`. The part that
 * reaches the end removes the export from the daemon's computer.
 */
export interface ReadDatabaseExportRequest {
	id: string;
	offset: number;
}

/** A connector account without its credential, safe to send to clients. */
export interface RedactedConnectorAccount {
	id: number;
	/** What serves the account. */
	host: AccountHost;
	/** The connector the account is one of: its name, icon, unit and kinds. */
	connector: Connector;
	/** What describes the account for people: its name or email. */
	description: string;
	/**
	 * When refreshing the account's credential last failed, in Unix
	 * seconds; absent while it works. The account needs signing in again.
	 */
	credential_failed_at: number | null;
	/** When the account was added, in Unix seconds. */
	created_at: number;
	/**
	 * Whether the account's items can be browsed and imported: its
	 * extension is granted `connector`. An orphaned account's can't.
	 */
	importable: boolean;
}

/** What serves an endpoint, without its API key. */
export type RedactedEndpointBackend =
	| {
			kind: 'integrated';
			/** The models.dev provider, such as `"anthropic"`. */
			provider_identifier: string;
			dialect: Dialect;
			base_url: string;
			/** The page where a person sees what the endpoint has used. */
			usage_url: string | null;
			/** How many minutes the server keeps a prompt cached. */
			cache_minutes: number;
			/**
			 * The longer cache an agent's requests ask for, or none when it
			 * offers none.
			 */
			long_cache: LongCache | null;
	  }
	| {
			kind: 'account';
			/** The models.dev provider, such as `"openai"`. */
			provider_identifier: string;
			dialect: Dialect;
			base_url: string;
			/** The connector account the endpoint signs in with. */
			account_id: number;
			/** The page where a person sees what the endpoint has used. */
			usage_url: string | null;
			/** How many minutes the server keeps a prompt cached. */
			cache_minutes: number;
			/**
			 * The longer cache an agent's requests ask for, or none when it
			 * offers none.
			 */
			long_cache: LongCache | null;
	  };

/** An intelligence endpoint without its API key, safe to send to clients. */
export interface RedactedIntelligenceEndpoint {
	id: number;
	/** What the endpoint is called. */
	name: string;
	/** What serves the endpoint. */
	backend: RedactedEndpointBackend;
}

/** An article after its page was fetched again. */
export interface RefetchedArticle {
	article: Article;
	/**
	 * Why this fetch failed, when it did. An article whose page was fetched
	 * before keeps that snapshot; one never fetched records the failure.
	 */
	failure: ArticleFetchFailure | null;
}

/** Reads live feed `feed_id` now, adding the items it hasn't seen. */
export interface RefreshFeedRequest {
	feed_id: number;
}

/** Refreshes every resource in workspace `workspace_id` from its service now. */
export interface RefreshResourcesRequest {
	workspace_id: number;
}

export interface RegisterExtensionRequest {
	script: string;
	allow_overwrite: boolean;
}

/**
 * One extension a registry offers, as its globals
 * `registryResults(query: string)` and
 * `registryLookup(identifiers: string[])` return them. The daemon installs from
 * the script at `sourceURL`, reading the manifest from that script; this
 * `manifest` is what the registry lists, shown before the user reviews it.
 */
export interface RegistryResult {
	/** The extension's manifest as the registry lists it. */
	manifest: ExtensionManifest;
	/** An `http` or `https` URL serving the extension's script. */
	source_url: string;
}

/** One registry's answer to a search. */
export interface RegistrySection {
	/** The registry extension's id. */
	extension_id: number;
	/** The registry extension's name. */
	name: string;
	results: RegistryResult[];
	/**
	 * Why this registry gave no results: it failed to load, threw, or
	 * returned something other than `RegistryResult[]`.
	 */
	error: string | null;
}

/**
 * The conversation a provider keeps for a thread, and how many of the
 * thread's messages it holds.
 */
export interface RemoteChain {
	remote_id: string;
	remote_cursor: number;
}

/**
 * Removes cue `cue_id`, so its hook no longer wakes the agent; an agent
 * left with no cues goes idle.
 */
export interface RemoveAgentCueRequest {
	cue_id: number;
}

/** Removes endpoint model `id`, unsetting any tier that uses it. */
export interface RemoveEndpointModelRequest {
	id: number;
}

/** Renames team `team_id`. */
export interface RenameAgentTeamRequest {
	team_id: number;
	/** Unique among teams, ignoring case. */
	title: string;
}

/** Retitles live article `article_id`. */
export interface RenameArticleRequest {
	article_id: number;
	title: string;
}

/** Renames client `id`. */
export interface RenameClientRequest {
	id: number;
	client_name: string;
}

/** Retitles document folder `folder_id` in a workspace. */
export interface RenameDocumentFolderRequest {
	workspace_id: number;
	folder_id: number;
	title: string;
}

/** Retitles live document `document_id` in a workspace. */
export interface RenameDocumentRequest {
	workspace_id: number;
	document_id: number;
	title: string;
}

/** Retitles live feed `feed_id`. */
export interface RenameFeedRequest {
	feed_id: number;
	title: string;
}

/** Renames saved view `view_id`. */
export interface RenameSavedTaskViewRequest {
	workspace_id: number;
	view_id: number;
	name: string;
}

/** Renames tag `tag_id` of workspace `workspace_id`. */
export interface RenameTagRequest {
	workspace_id: number;
	tag_id: number;
	name: string;
}

/** Retitles task folder `folder_id` in a workspace. */
export interface RenameTaskFolderRequest {
	workspace_id: number;
	folder_id: number;
	title: string;
}

/** Retitles task `task_id` in a workspace. */
export interface RenameTaskRequest {
	workspace_id: number;
	task_id: number;
	title: string;
}

/** Retitles workspace `id`. */
export interface RenameWorkspaceRequest {
	id: number;
	title: string;
}

/** Draws a Mermaid diagram. */
export interface RenderMermaidRequest {
	/** The diagram's Mermaid source: a fenced block's text, without the fence. */
	source: string;
	/**
	 * The width to lay out for, in points. A new width is a new layout, so a
	 * client asks once per width it shows rather than as it resizes. Left
	 * out, the diagram takes the size its content wants.
	 */
	width?: number | null;
	format: MermaidFormat;
}

/**
 * Asks the connector of resource `resource_id` in workspace `workspace_id`
 * for the CohesionUI interface it shows the resource with.
 */
export interface RenderResourceRequest {
	workspace_id: number;
	resource_id: number;
}

/** A diagram as drawn, or why it wasn't. */
export type RenderedMermaid =
	| {
			kind: 'svg';
			svg: string;
			/** The size it was laid out at, which its `viewBox` scales from. */
			width: number;
			height: number;
	  }
	| {
			kind: 'refused';
			reason: MermaidRefusal;
			/** Says what's wrong, in the renderer's words for a bad source. */
			message: string;
	  };

/** Puts the feeds in the order of `feed_ids`, first to last. */
export interface ReorderFeedsRequest {
	/** Every live feed, in its new order. */
	feed_ids: number[];
}

/** Puts the pinned agents in the order of `agent_ids`, first to last. */
export interface ReorderIntelligenceAgentsRequest {
	/** Every pinned agent, in its new order. */
	agent_ids: number[];
}

/**
 * Puts a workspace's saved task views in the order of `view_ids`, first to
 * last.
 */
export interface ReorderSavedTaskViewsRequest {
	workspace_id: number;
	/** Every view in the workspace, in its new order. */
	view_ids: number[];
}

/**
 * Puts workspace `workspace_id`'s tags in the order of `tag_ids`, first to
 * last.
 */
export interface ReorderTagsRequest {
	workspace_id: number;
	/** Every tag in the workspace, in its new order. */
	tag_ids: number[];
}

/** Puts a workspace's folders in the order of `folder_ids`, first to last. */
export interface ReorderTaskFoldersRequest {
	workspace_id: number;
	/** Every live folder in the workspace, in its new order. */
	folder_ids: number[];
}

/**
 * Puts the kinds shown in workspace `workspace_id` in the order of
 * `kind_ids`, first to last.
 */
export interface ReorderWorkspaceResourceKindsRequest {
	workspace_id: number;
	/** Every kind shown in the workspace, in its new order. */
	kind_ids: number[];
}

/** Fetches live article `article_id`'s page again and replaces its snapshot. */
export interface ResnapshotArticleRequest {
	article_id: number;
}

/**
 * Resolves actor session `session_id`: the user is done with it. One going on is
 * stopped first, telling its actor when its client is online. Lists leave
 * it out from then on, until the user says more, which starts it again.
 */
export interface ResolveActorSessionRequest {
	session_id: number;
}

/**
 * A read-only copy of an item imported into a workspace from a connected
 * service. The daemon refreshes it from the service periodically; scripts
 * cannot change it.
 */
export interface Resource {
	/** Unique, stable identifier. */
	id: number;
	/** Id of the workspace this resource was imported into. */
	workspace_id: number;
	/** Id of the connector account the resource came from. */
	account_id: number;
	/** The item's identifier in the connected service. */
	identifier: string;
	/** Id of the kind of item this is, one its connector declares. */
	kind_id: number;
	/**
	 * What sort of item this is, as `"<connector identifier>.<kind>"`, for
	 * example `"com.example.notion.page"`. Filter on it with
	 * `workspace.resources({ kind })`.
	 */
	kind: string;
	/** Display name. */
	title: string;
	/** Whether `contents` is Markdown, plain text or JSON text. */
	content_kind: ResourceContentKind;
	/**
	 * The item's contents: Markdown or plain text, or JSON text to
	 * `JSON.parse`, depending on the content kind.
	 */
	contents: string;
	/**
	 * When the item last changed in the connected service, in Unix
	 * **seconds** (in JavaScript use `new Date(lastUpdated * 1000)`).
	 */
	last_updated: number;
	/** When this copy last changed in Cohesion, in Unix **seconds**. */
	modified: number;
}

/**
 * What a resource's `contents` hold; stored as text.
 *
 * - `"markdown"`: Markdown text, searchable like a document.
 * - `"text"`: plain text, searchable like a document.
 * - `"json"`: a JSON document as text; parse it with `JSON.parse`. Only the
 *   title is searchable.
 */
export type ResourceContentKind = 'markdown' | 'text' | 'json';

/**
 * A kind of item a connector imports, as its definition last declared it.
 * A definition that drops a kind removes it, and its resources with it.
 */
export interface ResourceKind {
	/** Unique, stable identifier. */
	id: number;
	/** Id of the connector that declares it. */
	connector_id: number;
	/**
	 * The short key, such as `"page"`; a resource's `kind` is its
	 * connector's identifier and this.
	 */
	identifier: string;
	/** The kind's name for people, such as `"Page"`. */
	title: string;
	/** The name for several, such as `"Pages"`. */
	plural: string;
	/** What a resource's `contents` holds. */
	contents: string;
}

/** What a refresh pass did. */
export interface ResourceRefreshSummary {
	/** Resources fetched from their services. */
	checked: number;
	/** Resources whose contents changed. */
	updated: number;
	/** Resources that couldn't be fetched. */
	failed: number;
}

/**
 * The order a resource listing comes back in. Ties fall back to the
 * newest import, and resources with no value for a metadata key come last
 * in either direction.
 *
 * - `"title"`: alphabetical, ignoring case.
 * - `"lastUpdated"`: when the item last changed in the connected service.
 * - a metadata key: that key's value, as text ignoring case, so dates sort
 *   by time.
 */
export type ResourceSort = 'title' | 'lastUpdated' | MetadataKey;

/**
 * Picks option `option` (an index into its `options`) of the choice in
 * message `message_id`, which the reply follows, and queues the agent's
 * turn on the pick; like a message's, the reply doesn't wait for it. The
 * pick posts no message; the choice records it.
 */
export interface RespondToAgentChoiceRequest {
	message_id: number;
	option: number;
}

/**
 * Puts an item's text back to a kept version's (a task's status too),
 * bringing the item back if it was deleted, and keeps the result as a new
 * version. A deleted journal entry is refused while its day has another.
 */
export interface RestoreVersionRequest {
	workspace_id: number;
	version_id: number;
}

/** What restoring a version gave back: the item as it now is. */
export type RestoredItem =
	| {
			kind: 'task';
			task: Task;
	  }
	| {
			kind: 'document';
			document: Document;
	  };

/** Evaluates a script on the server. */
export interface RunScriptRequest {
	script: string;
}

/**
 * A way of looking at a workspace's tasks kept under a name: which tasks it
 * lists, the tags, priorities and sizes that narrow them, and the order they
 * come in.
 */
export interface SavedTaskView {
	id: number;
	workspace_id: number;
	/** Unique, ignoring case, among the workspace's views. */
	name: string;
	/** The tasks listed before `tags` narrow them. */
	scope: TaskViewScope;
	/** The tags a listed task must have, and those it must not. */
	tags: TagFilter;
	/** The priorities a listed task has one of; every priority when empty. */
	priorities: TaskPriorityList;
	/**
	 * The sizes a listed task has one of; every task, sized or not, when
	 * empty.
	 */
	sizes: TaskSizeList;
	sort: TaskSort;
	/** Reverses `sort`. */
	descending: boolean;
	/** Where the view sits among its workspace's views; lower comes first. */
	sort_order: number;
}

/**
 * Sets both dates of task `task_id` in a workspace, each a `"YYYY-MM-DD"`
 * day; `null` clears one. Any other text is a validation error.
 */
export interface ScheduleTaskRequest {
	workspace_id: number;
	task_id: number;
	start_date: string | null;
	due_date: string | null;
}

/**
 * The JSON text of a script's completion value. A newtype so that other
 * string replies can have their own `ReplyData` variant.
 */
export type ScriptResult = string;

/**
 * Searches every actor session's title and what the user and the actor said
 * in it, best match first. `query` is FTS5 syntax.
 */
export interface SearchActorSessionsRequest {
	query: string;
}

/**
 * Lists the items of kind `kind_id` connector account `account_id` offers
 * for import into workspace `workspace_id`, optionally narrowed to an
 * organization unit or a search.
 */
export interface SearchConnectorResourcesRequest {
	account_id: number;
	workspace_id: number;
	kind_id: number;
	organization_unit_id?: string | null;
	search?: string | null;
}

/** Lists the live documents in a workspace whose title contains `query`. */
export interface SearchDocumentRequest {
	workspace_id: number;
	query: string;
}

/**
 * What kind of item a search result points at: `"document"`, `"task"` or
 * `"resource"`.
 */
export type SearchEntityKind = 'document' | 'task' | 'resource';

/**
 * Asks every extension with the `registry` permission granted for the
 * extensions it offers matching `query` (`""` lists what it features).
 */
export interface SearchExtensionRegistriesRequest {
	query: string;
}

/**
 * One page of the full-text matches among the articles that came from
 * feeds, best first. `query` is FTS5 syntax.
 */
export interface SearchFeedCorpusRequest {
	query: string;
	/** Only articles from these feeds; every feed when absent. */
	feed_ids?: number[] | null;
	/** How many of the matches to skip; none when absent. */
	offset?: number;
	/** The most matches to list: 50 when absent, and never over 500. */
	limit?: number | null;
}

/**
 * One full-text search hit: a snapshot of the matched document's, task's or
 * resource's indexed text, plus a pointer back to it.
 */
export interface SearchResult {
	/**
	 * Id of the search index entry.
	 *
	 * This is **not** the matched item's id; that is the entity id.
	 */
	id: number;
	/** Whether the hit is a document, a task or a resource. */
	entity_kind: SearchEntityKind;
	/**
	 * Id of the matched document, task or resource (which one depends on the
	 * entity kind).
	 */
	entity_id: number;
	/** Id of the workspace the matched item belongs to. */
	workspace_id: number;
	/** The matched item's title. */
	title: string | null;
	/**
	 * The matched document's full contents or a Markdown resource's text;
	 * always `""` for tasks and JSON resources.
	 */
	contents: string;
}

/** Search the entire workspace. */
export interface SearchWorkspaceRequest {
	workspace_id: number;
	query: string;
}

/** Something that happened at the daemon's door. */
export interface SecurityEvent {
	id: number;
	/** When it happened, in Unix seconds. */
	created_at: number;
	kind: SecurityEventKind;
	/** The client it was about, if any. */
	client_id: number | null;
	/** That client's name at the time. */
	client_name: string | null;
	/**
	 * The address it came from, as the daemon saw it, if it came over the
	 * network.
	 */
	address: string | null;
}

/**
 * What a security event records.
 *
 * - `"pairingOpened"`: a client opened pairing.
 * - `"clientPaired"`: a new client paired.
 * - `"clientRenamed"`: a client was renamed.
 * - `"clientRevoked"`: a client was revoked.
 * - `"inactiveClientRevoked"`: a client was revoked for not connecting.
 * - `"settingsChanged"`: the security settings changed.
 * - `"pairingRefused"`: a pairing attempt was refused; recorded once an
 *   hour per address.
 */
export type SecurityEventKind =
	| 'pairingOpened'
	| 'clientPaired'
	| 'clientRenamed'
	| 'clientRevoked'
	| 'inactiveClientRevoked'
	| 'settingsChanged'
	| 'pairingRefused';

/** A page of the security activity. */
export interface SecurityEventPage {
	/** How many events there are in all, on any page. */
	total: number;
	events: SecurityEvent[];
}

/** The daemon's security settings; there is always exactly one row. */
export interface SecuritySettings {
	id: number;
	/** Which clients may open pairing. */
	pairing_opened_by: PairingOpener;
	/** Whether a new client pairing sends a notification. */
	alert_new_client: boolean;
	/**
	 * Revokes a client once it hasn't connected for this many days; never
	 * when `None`. A connected client is never revoked.
	 */
	revoke_inactive_after_days: number | null;
}

/**
 * Says `content` in actor session `session_id`, with any images in
 * `attachments`: more to do, which starts a session that is done or failed
 * again, or a response to one of its requests. Its client must be online.
 */
export interface SendActorSessionMessageRequest {
	session_id: number;
	content: ActorUserContent;
	attachments?: ImageUpload[] | null;
}

/**
 * Sends `message` to agent `agent_id` in its foreground chat, with any
 * images in `attachments` and the item `forward` names, which the message
 * then forwards. `message` may be empty when there are images or an item.
 * The message is saved before the reply, which comes at once. A turn the
 * agent is running takes it in before its next model call; otherwise the
 * agent's turn on it is queued behind any it is answering. What the turn posts
 * arrives with the agent's invalidations, and the agent has an `activity`
 * until it ends.
 */
export interface SendAgentMessageRequest {
	agent_id: number;
	message: string;
	attachments?: ImageUpload[] | null;
	forward?: AgentForwardTarget | null;
}

/** Why a command failed; `field` names the offending input when there is one. */
export interface ServerError {
	code: ErrorCode;
	message: string;
	field?: string | null;
}

/** One function of a [`ServiceManifest`]. */
export interface ServiceFunctionManifest {
	/** What the function does, and what its arguments mean. */
	description: string | null;
	/**
	 * The TypeScript type of the function's one argument, such as
	 * `"{ by?: number }"`. Leave it out for a function that takes none.
	 */
	args: string | null;
	/**
	 * The TypeScript type of what it returns, such as
	 * `"{ count: number }"`. Leave it out for `unknown`.
	 */
	returns: string | null;
	/**
	 * Whether the function only reads, changing nothing, so background
	 * agents may call it. Defaults to `false`.
	 */
	read_only: boolean | null;
}

/**
 * The `service` block of an [`ExtensionManifest`]. Scripts reach the
 * service by the extension's identifier, as
 * `services.use("<identifier>")`.
 */
export interface ServiceManifest {
	/** What the service offers, in a sentence or two. */
	description: string | null;
	/** The functions, by name; each name is a JavaScript identifier. */
	functions: Record<string, ServiceFunctionManifest>;
}

/**
 * Switches actor session `session_id` to mode `mode`, one its actor offers, from
 * its next step on, even while it works. Its client must be online.
 */
export interface SetActorSessionModeRequest {
	session_id: number;
	mode: string;
}

/**
 * Reacts to message `message_id`, one the agent sent, with a single emoji
 * (clients offer those in `AGENT_REACTIONS`); a blank or absent `reaction`
 * clears it. A reaction queues the agent's turn on it, which may post
 * nothing; like a message's, the reply doesn't wait for it.
 */
export interface SetAgentMessageReactionRequest {
	message_id: number;
	reaction?: string | null;
}

/**
 * Pins or unpins agent `agent_id`; a newly pinned agent goes after the
 * other pinned agents.
 */
export interface SetAgentPinnedRequest {
	agent_id: number;
	pinned: boolean;
}

/**
 * Puts agent `agent_id` on team `team_id`, or on none when absent. Its pin
 * is left as it was.
 */
export interface SetAgentTeamRequest {
	agent_id: number;
	team_id?: number | null;
}

/** Marks live article `article_id` read or unread. */
export interface SetArticleReadRequest {
	article_id: number;
	read: boolean;
}

/**
 * Sets live contact `contact_id`'s photo in workspace `workspace_id`,
 * replacing any it had, or removes it when `photo` is `null`. The photo
 * passes the same check as an image attached to a message.
 */
export interface SetContactPhotoRequest {
	workspace_id: number;
	contact_id: number;
	photo: ImageUpload | null;
}

/**
 * Enables or disables `hook` for extension `extension_id`. The extension's
 * running sandbox is discarded, and enabling `launch` calls `onLaunch`.
 */
export interface SetExtensionHookRequest {
	extension_id: number;
	hook: HookKind;
	enabled: boolean;
}

/**
 * Enables or disables `permission` for extension `extension_id`. The
 * extension's running sandbox is discarded, so the change applies at once.
 */
export interface SetExtensionPermissionRequest {
	extension_id: number;
	permission: ExtensionPermissionKind;
	enabled: boolean;
}

/**
 * Sets when live feed `feed_id`'s items are read from their pages. Items
 * already added keep their text.
 */
export interface SetFeedFullPagePolicyRequest {
	feed_id: number;
	full_page_policy: FeedFullPagePolicy;
}

/** Makes endpoint model `endpoint_model_id` the default for `tier`. */
export interface SetIntelligenceDefaultRequest {
	tier: IntelligenceTier;
	endpoint_model_id: number;
}

/**
 * Sets how many minutes endpoint `id`'s server keeps a prompt cached, at
 * least 1.
 */
export interface SetIntelligenceEndpointCacheMinutesRequest {
	id: number;
	cache_minutes: number;
}

/**
 * Sets the longer prompt cache endpoint `id`'s server offers, which an
 * agent's requests ask for, or with no `long_cache` removes it.
 */
export interface SetIntelligenceEndpointLongCacheRequest {
	id: number;
	long_cache: LongCache | null;
}

/**
 * Sets where a person sees what endpoint `id` has used, or with no
 * `usage_url` removes it.
 */
export interface SetIntelligenceEndpointUsageUrlRequest {
	id: number;
	usage_url: string | null;
}

/**
 * Limits what every model call together may cost over `period` to
 * `amount` US dollars, more than zero, replacing its limit if it has one.
 */
export interface SetIntelligenceSpendLimitRequest {
	period: SpendPeriod;
	amount: number;
}

/**
 * Sets the user's bio, without the spaces around it, or removes it when it
 * is `null` or empty. A bio past 1,000 characters is refused.
 */
export interface SetProfileBioRequest {
	bio: string | null;
}

/**
 * Sets the user's name, without the spaces around it, or removes it when
 * it is `null` or empty.
 */
export interface SetProfileNameRequest {
	name: string | null;
}

/**
 * Sets the user's photo, replacing any they had, or removes it when
 * `photo` is `null`. The photo passes the same check as an image attached
 * to a message.
 */
export interface SetProfilePhotoRequest {
	photo: ImageUpload | null;
}

/** Shows kind `kind_id` in workspace `workspace_id`, or hides it there. */
export interface SetResourceKindShownRequest {
	workspace_id: number;
	kind_id: number;
	shown: boolean;
}

/**
 * Puts tag `tag_id` of workspace `workspace_id` on the workspace's task
 * `task_id`, or takes it off.
 */
export interface SetTaggedRequest {
	workspace_id: number;
	tag_id: number;
	task_id: number;
	tagged: boolean;
}

/**
 * Assigns task `task_id` in a workspace to the user or one of their
 * agents; `null` leaves it unassigned.
 */
export interface SetTaskAssigneeRequest {
	workspace_id: number;
	task_id: number;
	assignee: TaskAssignee | null;
}

/** Sets the priority of task `task_id` in a workspace. */
export interface SetTaskPriorityRequest {
	workspace_id: number;
	task_id: number;
	priority: TaskPriority;
}

/** Sets the size of task `task_id` in a workspace; `null` clears it. */
export interface SetTaskSizeRequest {
	workspace_id: number;
	task_id: number;
	size: TaskSize | null;
}

/** Sets workspace `id`'s emoji, or clears it when `emoji` is absent. */
export interface SetWorkspaceEmojiRequest {
	id: number;
	emoji?: string | null;
}

/**
 * A task size, its name and the color it is drawn in. The colors are a set
 * of their own, none of them the ones priorities use, so a list can be scanned
 * for either.
 */
export interface SizeWording {
	kind: TaskSize;
	label: string;
	color: IntelligenceAgentColor;
}

/**
 * How far back a spend limit counts model calls. Each is rolling: it
 * counts what was spent in the length of time just past, so spending
 * falls back under a limit as old calls age out, not at a set hour.
 *
 * - `"hour"`: the last 60 minutes.
 * - `"day"`: the last 24 hours.
 * - `"month"`: the last 30 days.
 */
export type SpendPeriod = 'hour' | 'day' | 'month';

/**
 * Hands actor `actor_id` a new session, `prompt`, with any images in
 * `attachments`, in the mode, on the model and at the effort given (the
 * actor's defaults when left out), titled `title`, or by the prompt's
 * first line when left out. Its client must be online.
 */
export interface StartActorSessionRequest {
	actor_id: number;
	prompt: string;
	title?: string | null;
	mode?: string | null;
	model?: string | null;
	effort?: string | null;
	attachments?: ImageUpload[] | null;
}

/**
 * Stops what actor session `session_id` is doing, telling its actor when its
 * client is online; the session is idle, until the user says more.
 */
export interface StopActorSessionRequest {
	session_id: number;
}

/**
 * Stops the running turn of agent `agent_id`'s conversation: a script it
 * runs is interrupted and shows as cancelled, and the turn ends without
 * another model call. Nothing happens when no turn is running.
 */
export interface StopAgentTurnRequest {
	agent_id: number;
}

/**
 * A part of what Cohesion keeps, as Settings › Storage breaks the
 * database's size down.
 */
export type StorageArea =
	| 'agentChats'
	| 'actorSessions'
	| 'documentsAndTasks'
	| 'feedsAndArticles'
	| 'resources'
	| 'memories'
	| 'rewindVersions'
	| 'other';

/** One area's share of the database file. */
export interface StorageAreaSize {
	area: StorageArea;
	/** The pages its tables and indexes take up, in bytes. */
	bytes: number;
}

/**
 * The daemon's storage settings; there is always exactly one row. The
 * hourly cleanup applies the retentions, and remembering applies the
 * memory limit. Every value is at least one.
 */
export interface StorageSettings {
	id: number;
	/**
	 * A model thread nothing has been added to for this many days is
	 * deleted, and a thread message or a script's result in an agent's chat
	 * this old loses its contents.
	 */
	thread_retention_days: number;
	/**
	 * An actor session resolved this many days ago is deleted, with its images;
	 * one never resolved stays.
	 */
	actor_session_retention_days: number;
	/**
	 * A kept version of a task or document last changed this many days ago
	 * is deleted, and with the last of them a deleted item's way back.
	 */
	rewind_retention_days: number;
	/**
	 * A deleted task, document or journal entry is removed for good this many
	 * days after it was deleted, or once Rewind's retention has passed if that
	 * is longer.
	 */
	deleted_item_retention_days: number;
	/**
	 * The most memories each scope keeps: an agent its own, a team its
	 * own, and every agent's together. Lowering it forgets nothing; past
	 * it, remembering another in that scope is refused.
	 */
	memory_limit: number;
}

/** A name a workspace puts on its tasks, to group them across folders. */
export interface Tag {
	/** Unique, stable identifier. */
	id: number;
	/** Id of the workspace the tag belongs to. */
	workspace_id: number;
	/** Unique, ignoring case, among the workspace's tags. */
	name: string;
	/** Position among the workspace's tags, first to last. */
	sort_order: number;
	/** In Unix seconds. */
	created_at: number;
}

/**
 * Which tags the items a listing covers must carry, and which they must
 * not: an item is listed when it has every tag in `with` and none in
 * `without`. Empty lists filter nothing.
 */
export interface TagFilter {
	with: number[];
	without: number[];
}

/** The ids of the tags on a task; read from a JSON array. */
export type TagIds = number[];

/** A to-do item in a workspace, optionally filed in a task folder. */
export interface Task {
	/** Unique, stable identifier. */
	id: number;
	/** Id of the workspace this task belongs to. */
	workspace_id: number;
	/**
	 * Id of the task folder this task is filed in, or `null` when it is not
	 * in a folder.
	 */
	task_folder_id: number | null;
	/** What the task is. */
	title: string;
	/**
	 * Notes on the task, as Markdown; `""` when it has none.
	 *
	 * Searched along with the title.
	 */
	description: string;
	/** Where the task stands. */
	status: TaskStatus;
	/** How urgent the task is. */
	priority: TaskPriority;
	/**
	 * How much time or effort the task takes, from 1 to 5.
	 *
	 * `null` when it has none.
	 */
	size: TaskSize | null;
	/** Who the task is assigned to; `null` when no one is. */
	assignee: TaskAssignee | null;
	/**
	 * The day work on the task can begin, as `"YYYY-MM-DD"`; a calendar day,
	 * the same wherever it is read.
	 *
	 * `null` when it has none.
	 */
	start_date: string | null;
	/**
	 * The day the task is due, in the same form as `start_date`.
	 *
	 * `null` when it has none.
	 */
	due_date: string | null;
	/** Whether the task has been soft-deleted. */
	deleted: boolean;
	/** Ids of the tags on the task. */
	tag_ids: TagIds;
}

/**
 * Who a task is assigned to: the user, one of their agents, or one of the
 * task's workspace's contacts.
 *
 * - `"user"`: the user.
 * - `"agent"`: agent `agent_id`. Deleting the agent leaves the task
 *   unassigned.
 * - `"contact"`: contact `contact_id`, of the task's workspace. Deleting the
 *   contact leaves the task unassigned.
 */
export type TaskAssignee =
	| { kind: 'user' }
	| {
			kind: 'agent';
			agent_id: number;
	  }
	| {
			kind: 'contact';
			contact_id: number;
	  };

/** A named group of tasks in a workspace. */
export interface TaskFolder {
	id: number;
	workspace_id: number;
	title: string;
	/** Where the folder sits among its workspace's folders; lower comes first. */
	sort_order: number;
	deleted: boolean;
}

/**
 * Which folders a task listing covers.
 *
 * - `"any"`: every task, filed or not (the default).
 * - `"inbox"`: only tasks in no folder.
 * - a folder id: only tasks filed in that folder.
 */
export type TaskFolderFilter = 'any' | 'inbox' | number;

/** One page of a listing: its tasks, and how many the whole listing has. */
export interface TaskPage {
	tasks: Task[];
	/** Every task the listing covers, on any page. */
	total: number;
}

/**
 * How urgent a task is; stored as a number so tasks sort by it, `"none"`
 * lowest.
 *
 * Unknown stored numbers read as `"none"`.
 *
 * - `"none"`: no priority set (new tasks start here).
 * - `"low"`
 * - `"medium"`
 * - `"high"`
 * - `"urgent"`
 */
export type TaskPriority = 'none' | 'low' | 'medium' | 'high' | 'urgent';

/** The priorities a saved view lists; read from a JSON array of their numbers. */
export type TaskPriorityList = TaskPriority[];

/**
 * How much time or effort a task takes, from 1 (the least) to 5 (the
 * most); a bare number on the wire and in the database.
 *
 * Clients name each size with the vocabulary's `task_sizes`, never their
 * own words, so the names can change without the numbers changing.
 */
export type TaskSize = 1 | 2 | 3 | 4 | 5;

/** The sizes a saved view lists; read from a JSON array. */
export type TaskSizeList = TaskSize[];

/**
 * The order a task listing comes back in.
 *
 * Ties fall back to creation order, and tasks with no value for the key (no
 * size, start or due day) come last in either direction.
 *
 * - `"created"`: oldest first (the default).
 * - `"title"`: alphabetical, ignoring case.
 * - `"priority"`: `"none"` first, `"urgent"` last.
 * - `"size"`: smallest first.
 * - `"startDate"`: earliest start day first.
 * - `"dueDate"`: earliest due day first.
 */
export type TaskSort = 'created' | 'title' | 'priority' | 'size' | 'startDate' | 'dueDate';

/**
 * Where a task stands; stored as text.
 *
 * Unknown stored text reads as `"incomplete"`.
 *
 * - `"incomplete"`: still to do (new tasks start here).
 * - `"completed"`: done.
 * - `"cancelled"`: will not be done.
 */
export type TaskStatus = 'incomplete' | 'completed' | 'cancelled';

/**
 * The tasks a saved view lists: one of the lists every workspace has, or
 * the open tasks of one of its folders. On the wire, a list's name or the
 * folder's id.
 */
export type TaskViewScope = 'inbox' | 'all' | 'done' | 'cancelled' | number;

/** `kind: "text"`: a paragraph of plain text. */
export interface TextBlock {
	/** The text, shown as written; line breaks are kept. */
	text: string;
}

/**
 * What a thread message says, tagged by `kind`.
 *
 * - `"userMessage"`: what the model was told.
 * - `"assistantMessage"`: what the model said.
 * - `"thought"`: the model's reasoning. `signature` is an opaque token some
 *   providers require back with it; `in_content` says it came inside the
 *   message content, which is how it goes back.
 * - `"redactedThought"`: reasoning the provider withheld: an opaque `trace`,
 *   and a summary if it gave one.
 * - `"toolCallRequest"`: a tool the model called; `arguments` is what the
 *   model sent, usually JSON.
 * - `"toolCallResponse"`: what the call returned, or why it failed.
 *
 * An old message is emptied but stays this shape, so a request still pairs
 * with its response.
 */
export type ThreadItem =
	| {
			kind: 'userMessage';
			text: string;
	  }
	| {
			kind: 'assistantMessage';
			text: string;
	  }
	| {
			kind: 'thought';
			text: string;
			signature: string | null;
			in_content: boolean;
	  }
	| {
			kind: 'redactedThought';
			trace: string;
			summary: string | null;
	  }
	| {
			kind: 'toolCallRequest';
			call_identifier: string;
			tool_name: string;
			arguments: string;
	  }
	| {
			kind: 'toolCallResponse';
			call_identifier: string;
			succeeded: boolean;
			text: string;
	  };

/**
 * Where a thread's first messages come from.
 *
 * - `"root"`: it holds all its own.
 * - `"fork"`: it carries on another thread: every message of `parent_thread_id`
 *   through `inherited_through_id` comes before its own, read from the parent
 *   rather than copied, and there are `inherited_items` of them, counting
 *   what the parent itself inherited.
 */
export type ThreadOrigin =
	| { kind: 'root' }
	| {
			kind: 'fork';
			parent_thread_id: number;
			inherited_through_id: number;
			inherited_items: number;
	  };

/**
 * Removes extension `extension_id`, stopping its sandbox and timers and
 * dropping its permission and hook grants.
 */
export interface UninstallExtensionRequest {
	extension_id: number;
}

/**
 * Replaces contact method `method_id`'s kind and value, which may not be
 * empty, on a live contact in workspace `workspace_id`.
 */
export interface UpdateContactMethodRequest {
	workspace_id: number;
	method_id: number;
	kind: ContactMethodKind;
	value: string;
}

/**
 * Replaces live contact `contact_id`'s names and notes in workspace
 * `workspace_id`, by the same rules as creating one.
 */
export interface UpdateContactRequest {
	workspace_id: number;
	contact_id: number;
	first_name: string;
	middle_name?: string | null;
	last_name: string;
	notes: string;
}

/** Changes agent `agent_id`; an absent field is left alone. */
export interface UpdateIntelligenceAgentRequest {
	agent_id: number;
	name?: string | null;
	color?: IntelligenceAgentColor | null;
	/** The new emoji; `""` clears it. */
	emoji?: string | null;
	personality?: IntelligenceAgentPersonality | null;
	verbosity?: IntelligenceAgentVerbosity | null;
	/** The new role; `""` clears it. */
	role?: string | null;
	/** The new directive; `""` clears it. */
	directive?: string | null;
}

/**
 * Replaces what saved view `view_id` lists, and in what order, keeping its
 * name and place.
 */
export interface UpdateSavedTaskViewRequest {
	workspace_id: number;
	view_id: number;
	scope: TaskViewScope;
	tags: TagFilter;
	/** Every priority when empty. */
	priorities: TaskPriority[];
	/** Every task, sized or not, when empty or absent. */
	sizes?: TaskSize[];
	sort: TaskSort;
	descending: boolean;
}

/** Replaces the security settings. */
export interface UpdateSecuritySettingsRequest {
	pairing_opened_by: PairingOpener;
	alert_new_client: boolean;
	/** Never when `None`; at least one day. */
	revoke_inactive_after_days: number | null;
}

/** Replaces the storage settings. */
export interface UpdateStorageSettingsRequest {
	/** Each at least one. */
	thread_retention_days: number;
	actor_session_retention_days: number;
	rewind_retention_days: number;
	deleted_item_retention_days: number;
	memory_limit: number;
}

/**
 * An account several actors draw on, which one set of plan limits fuels:
 * the actors reporting the same `id`, from any client, share them, and
 * the latest report among them stands for all.
 */
export interface UsagePool {
	/**
	 * Names the account for good, the same from every actor drawing on
	 * it, prefixed with the product to keep apart other products' ids:
	 * `claude:jackson@example.com`.
	 */
	id: string;
	/** The account, as people read it: `jackson@example.com`. */
	name: string;
}

/** A task's or document's text at one moment. */
export interface Version {
	/** Unique, stable identifier. */
	id: number;
	/** What it is a version of, and for a task, its status then. */
	item: VersionedItem;
	/** Id of the workspace the item belongs to. */
	workspace_id: number;
	/** The item's title then. */
	title: string;
	/** A task's description, or a document's full text, then. */
	body: string;
	/** What this version records. */
	change: VersionChange;
	/** Who made it, or last edited it. */
	author: VersionAuthor;
	/** The agent's or extension's name at the time, if `author` is either. */
	author_name: string | null;
	/** When this version began, in Unix **seconds**. */
	created: number;
	/** When it last changed, in Unix **seconds**: its last edit. */
	modified: number;
}

/**
 * Who made a kept version; stored as text.
 *
 * - `"user"`: the user, from any client.
 * - `"agent"`: an agent, named by the version's `author_name`.
 * - `"extension"`: an extension, named by the version's `author_name`.
 * - `"script"`: a script run by hand, outside any extension or agent.
 * - `"unknown"`: nobody on record, for an `"earlier"` version.
 */
export type VersionAuthor = 'user' | 'agent' | 'extension' | 'script' | 'unknown';

/**
 * What a kept version records; stored as text.
 *
 * - `"created"`: the item as it was made.
 * - `"edited"`: the item after an edit. Edits within two minutes of a
 *   version's start share it, whoever makes them.
 * - `"deleted"`: the item as it was when it was deleted.
 * - `"restored"`: the item after an older version was restored.
 * - `"earlier"`: the item before Rewind has a record of who changed it;
 *   the first change after its older versions have gone keeps it.
 */
export type VersionChange = 'created' | 'edited' | 'deleted' | 'restored' | 'earlier';

/** A kept version without its text: a row of an item's history. */
export interface VersionSummary {
	/** Unique, stable identifier. */
	id: number;
	/** What it is a version of, and for a task, its status then. */
	item: VersionedItem;
	/** Id of the workspace the item belongs to. */
	workspace_id: number;
	/** What this version records. */
	change: VersionChange;
	/** Who made it, or last edited it. */
	author: VersionAuthor;
	/** The agent's or extension's name at the time, if `author` is either. */
	author_name: string | null;
	/** When it last changed, in Unix **seconds**: its last edit. */
	modified: number;
	/**
	 * Whether this is what the item holds now; `false` for every version of
	 * a deleted item, which restoring any of them brings back.
	 */
	current: boolean;
}

/** Which item a history is asked of. */
export type VersionTarget =
	| {
			kind: 'task';
			task_id: number;
	  }
	| {
			kind: 'document';
			document_id: number;
	  };

/**
 * What a kept version is of, and what only that kind of item keeps. Real
 * foreign keys, so an item's versions cascade with it, and the table's checks
 * hold a version to exactly one.
 *
 * - `"task"`: a task, with its status then.
 * - `"document"`: a document or journal entry.
 */
export type VersionedItem =
	| {
			kind: 'task';
			task_id: number;
			status: TaskStatus;
	  }
	| {
			kind: 'document';
			document_id: number;
	  };

/** Every kind a client names, each list in the order clients show it. */
export interface Vocabulary {
	/** Extension permissions. */
	permissions: { kind: ExtensionPermissionKind; title: string; description: string }[];
	hooks: HookWording[];
	/** Most to least pressing. */
	task_priorities: { kind: TaskPriority; label: string }[];
	/** What each kind of version Rewind keeps records. */
	version_changes: { kind: VersionChange; label: string }[];
	/** Smallest to largest. */
	task_sizes: SizeWording[];
	activity_statuses: { kind: ActivityStatus; label: string }[];
	/** An actor session's statuses: idle, then its activity's. */
	actor_session_statuses: { kind: ActorSessionStatus; label: string }[];
	/** How an actor session stopped on its own, as a wake line says it. */
	actor_session_stops: { kind: ActorSessionStop; label: string }[];
	agent_colors: { kind: IntelligenceAgentColor; label: string }[];
	agent_personalities: { kind: IntelligenceAgentPersonality; label: string }[];
	agent_verbosities: { kind: IntelligenceAgentVerbosity; label: string }[];
	/** Model tiers, with what each is used for. */
	tiers: { kind: IntelligenceTier; title: string; description: string }[];
	/** Spend limit periods, shortest first, with what each counts. */
	spend_periods: { kind: SpendPeriod; title: string; description: string }[];
	/** Providers with a built-in endpoint preset. */
	providers: ProviderWording[];
	dialects: { kind: Dialect; label: string }[];
	/** The emoji an agent message can be reacted with. */
	agent_reactions: string[];
	/** Which clients may open pairing. */
	pairing_openers: { kind: PairingOpener; label: string }[];
	/** What each kind of security activity is called. */
	security_events: { kind: SecurityEventKind; label: string }[];
}

/** The web UI's static build, for a client to serve. */
export interface WebBundle {
	/**
	 * Identifies this build: any change to the files changes it, so a client
	 * may keep what it unpacked for as long as the hash is unchanged.
	 */
	hash: string;
	files: WebFile[];
}

/** The hash of the web UI the daemon was built with. */
export interface WebBundleHash {
	/**
	 * The [`WebBundle`]'s hash, the same value as `Hello`'s
	 * `web_bundle_hash`; `None` when the daemon was built without the UI.
	 */
	hash: string | null;
}

/** One file of the web UI's build. */
export interface WebFile {
	/** Relative to the build's root, with `/` separators, e.g. `_app/version.json`. */
	path: string;
	/**
	 * The file's bytes, compressed as raw deflate (RFC 1951, no zlib or gzip
	 * wrapper), base64-encoded.
	 */
	data: string;
}

/** A top-level container for documents and tasks. */
export interface Workspace {
	/** Unique, stable identifier. */
	id: number;
	/**
	 * Display name.
	 *
	 * Not guaranteed unique; looking a workspace up by title is an exact,
	 * case-sensitive match.
	 */
	title: string;
	/** An emoji shown in place of the title's initial; none when unset. */
	emoji: string | null;
	/**
	 * Whether the workspace has been soft-deleted.
	 *
	 * Lookups and listings skip deleted workspaces, so this is normally
	 * `false`.
	 */
	deleted: boolean;
}

/**
 * A kind of resource shown in a workspace. A kind is hidden in every
 * workspace until it is shown there; hidden, its resources stay.
 */
export interface WorkspaceResourceKind {
	id: number;
	workspace_id: number;
	kind_id: number;
	/**
	 * Where the kind sits among those the workspace shows; a kind shown
	 * later goes last.
	 */
	sort_order: number;
}

/**
 * Adds `data` to import `id`. `offset` is how much it holds already, so a
 * part sent twice or out of order is refused.
 */
export interface WriteDatabaseImportRequest {
	id: string;
	offset: number;
	data: string;
}

/** Every request `type` and the shape of its `args`. */
export interface Requests {
	about: NoArgs;
	/** Answers the web UI this cohesiond was built with, for a client to serve. */
	webBundle: NoArgs;
	/**
	 * Answers the hash of the web UI this cohesiond was built with, so a
	 * client can check its copy without fetching the files.
	 */
	webBundleHash: NoArgs;
	/**
	 * Opens a pairing window and answers the setup code a new client
	 * registers with. It works for one registration, for ten minutes.
	 * Refused to a client the security settings don't let open pairing.
	 */
	beginPairing: NoArgs;
	/** Answers whether the client asking may open pairing now, and why not. */
	getPairingAccess: NoArgs;
	/** Evaluates a script on the server. */
	runScript: RunScriptRequest;
	/** Draws a Mermaid diagram, or says why it can't be drawn. */
	renderMermaid: RenderMermaidRequest;
	/** Lists every workspace that is not deleted. */
	listWorkspaces: NoArgs;
	/** Creates a workspace with `title`. */
	createWorkspace: CreateWorkspaceRequest;
	/** Deletes workspace `id`. */
	deleteWorkspace: DeleteWorkspaceRequest;
	/** Retitles workspace `id`. */
	renameWorkspace: RenameWorkspaceRequest;
	/** Sets or clears a workspace's emoji. */
	setWorkspaceEmoji: SetWorkspaceEmojiRequest;
	/** Fetches the journal for a date in a workspace, if it exists. */
	getJournal: GetJournalRequest;
	/** Appends `content` to the journal for a date in a workspace. */
	appendToJournal: AppendToJournalRequest;
	/** Replaces the contents of the journal for a date in a workspace. */
	editJournal: EditJournalRequest;
	/** Search documents */
	searchDocuments: SearchDocumentRequest;
	/** Lists the live documents in a workspace, most recently modified first. */
	listDocuments: ListDocumentsRequest;
	/** Fetches live document `document_id` in a workspace. */
	getDocument: GetDocumentRequest;
	/** Replaces the contents of live document `document_id` in a workspace. */
	editDocument: EditDocumentRequest;
	/** Creates an ordinary document in a workspace. */
	createDocument: CreateDocumentRequest;
	/** Retitles live document `document_id` in a workspace. */
	renameDocument: RenameDocumentRequest;
	/** Deletes document `document_id` in a workspace. */
	deleteDocument: DeleteDocumentRequest;
	/** Files a document into a folder, or the Inbox. */
	moveDocument: MoveDocumentRequest;
	/** Lists a workspace's document folders. */
	listDocumentFolders: ListDocumentFoldersRequest;
	/** Creates a document folder, at the top level or inside another. */
	createDocumentFolder: CreateDocumentFolderRequest;
	/** Retitles a document folder. */
	renameDocumentFolder: RenameDocumentFolderRequest;
	/**
	 * Moves a document folder into another, or the top level, and among its
	 * new siblings; replies with every folder in the workspace.
	 */
	moveDocumentFolder: MoveDocumentFolderRequest;
	/** Deletes a document folder and the folders inside it. */
	deleteDocumentFolder: DeleteDocumentFolderRequest;
	/** Lists an item's kept versions, newest first, without their text. */
	listVersions: ListVersionsRequest;
	/** Fetches one kept version. */
	getVersion: GetVersionRequest;
	/**
	 * Puts an item back to a kept version, bringing it back if it was
	 * deleted, and keeps the result as a new version.
	 */
	restoreVersion: RestoreVersionRequest;
	/** Lists the deleted items Rewind can still bring back. */
	listDeletedItems: ListDeletedItemsRequest;
	/** Lists task folders */
	listTaskFolders: ListTaskFoldersRequest;
	createTaskFolder: CreateTaskFolderRequest;
	deleteTaskFolder: DeleteTaskFolderRequest;
	/** Retitles task folder `folder_id` in a workspace. */
	renameTaskFolder: RenameTaskFolderRequest;
	/** Reorders a workspace's task folders; replies with them in their new order. */
	reorderTaskFolders: ReorderTaskFoldersRequest;
	/** Lists a workspace's saved task views, in their order. */
	listSavedTaskViews: ListSavedTaskViewsRequest;
	/** Saves a way of looking at a workspace's tasks under a name. */
	createSavedTaskView: CreateSavedTaskViewRequest;
	/** Replaces what saved task view `view_id` lists, and in what order. */
	updateSavedTaskView: UpdateSavedTaskViewRequest;
	/** Renames saved task view `view_id` in a workspace. */
	renameSavedTaskView: RenameSavedTaskViewRequest;
	/** Reorders a workspace's saved task views; replies with them in their new order. */
	reorderSavedTaskViews: ReorderSavedTaskViewsRequest;
	/** Deletes saved task view `view_id` in a workspace. */
	deleteSavedTaskView: DeleteSavedTaskViewRequest;
	/** Lists a workspace's tags, in their order. */
	listTags: ListTagsRequest;
	/** Creates a tag in a workspace. */
	createTag: CreateTagRequest;
	/** Renames tag `tag_id` in a workspace. */
	renameTag: RenameTagRequest;
	/** Reorders a workspace's tags; replies with them in their new order. */
	reorderTags: ReorderTagsRequest;
	/** Deletes tag `tag_id` in a workspace, taking it off everything. */
	deleteTag: DeleteTagRequest;
	/** Puts a tag on a task or takes it off; replies with the task. */
	setTagged: SetTaggedRequest;
	/** Lists a workspace's live contacts, by last name, then first name. */
	listContacts: ListContactsRequest;
	/** Fetches live contact `contact_id` in a workspace. */
	getContact: GetContactRequest;
	/** Creates a contact in a workspace. */
	createContact: CreateContactRequest;
	/** Replaces contact `contact_id`'s names and notes. */
	updateContact: UpdateContactRequest;
	/** Deletes contact `contact_id` in a workspace. */
	deleteContact: DeleteContactRequest;
	/** Adds an email address or phone number to contact `contact_id`. */
	addContactMethod: AddContactMethodRequest;
	/** Replaces contact method `method_id`'s kind and value. */
	updateContactMethod: UpdateContactMethodRequest;
	/** Removes contact method `method_id` from its contact. */
	deleteContactMethod: DeleteContactMethodRequest;
	/** Sets or removes contact `contact_id`'s photo. */
	setContactPhoto: SetContactPhotoRequest;
	/** Lists a page of the live tasks the filters cover. */
	listTasks: ListTasksRequest;
	createTask: CreateTaskRequest;
	/** Retitles task `task_id` in a workspace. */
	renameTask: RenameTaskRequest;
	/** Sets the priority of task `task_id` in a workspace. */
	setTaskPriority: SetTaskPriorityRequest;
	/** Sets the size of task `task_id` in a workspace; `null` clears it. */
	setTaskSize: SetTaskSizeRequest;
	/**
	 * Assigns task `task_id` in a workspace to the user or one of their
	 * agents; `null` leaves it unassigned.
	 */
	setTaskAssignee: SetTaskAssigneeRequest;
	/** Replaces the description of task `task_id` in a workspace; `""` clears it. */
	editTaskDescription: EditTaskDescriptionRequest;
	/** Sets both dates of task `task_id` in a workspace; `null` clears one. */
	scheduleTask: ScheduleTaskRequest;
	/** Moves task `task_id` into folder `folder_id`, or the inbox when absent. */
	moveTask: MoveTaskRequest;
	/** Deletes task `task_id` in a workspace. */
	deleteTask: DeleteTaskRequest;
	searchWorkspace: SearchWorkspaceRequest;
	documentation: NoArgs;
	updateModelCatalog: NoArgs;
	/** Lists every intelligence endpoint. */
	listIntelligenceEndpoints: NoArgs;
	createIntelligenceEndpoint: CreateIntelligenceEndpointRequest;
	deleteIntelligenceEndpoint: DeleteIntelligenceEndpointRequest;
	/** Sets or removes where a person sees what an endpoint has used. */
	setIntelligenceEndpointUsageUrl: SetIntelligenceEndpointUsageUrlRequest;
	/** Sets how long an endpoint's server keeps a prompt cached. */
	setIntelligenceEndpointCacheMinutes: SetIntelligenceEndpointCacheMinutesRequest;
	/** Sets or removes the longer prompt cache an endpoint's server offers. */
	setIntelligenceEndpointLongCache: SetIntelligenceEndpointLongCacheRequest;
	/** Imports a model into an endpoint, from the catalog when it lists it. */
	addEndpointModel: AddEndpointModelRequest;
	/** Removes an endpoint's model, unsetting any tier that uses it. */
	removeEndpointModel: RemoveEndpointModelRequest;
	/** Lists the models an endpoint offers. */
	listEndpointModels: ListEndpointModelsRequest;
	/** Makes an endpoint model the default for a tier. */
	setIntelligenceDefault: SetIntelligenceDefaultRequest;
	listIntelligenceDefaults: NoArgs;
	/** Removes the default model of `tier`. */
	clearIntelligenceDefault: ClearIntelligenceDefaultRequest;
	/** Lists what model calls cost over each spend period, against its limit. */
	listIntelligenceSpend: NoArgs;
	/** Sets the spend limit of a period, answering every period's spend. */
	setIntelligenceSpendLimit: SetIntelligenceSpendLimitRequest;
	/** Removes the spend limit of a period, answering every period's spend. */
	clearIntelligenceSpendLimit: ClearIntelligenceSpendLimitRequest;
	listCatalogModels: ListCatalogModelsRequest;
	/** Fetches intelligence thread `thread_id` with its messages. */
	getIntelligenceThread: GetIntelligenceThreadRequest;
	/** Lists the `limit` most recent intelligence cost events, newest first. */
	listIntelligenceCostEvents: ListIntelligenceCostEventsRequest;
	changeTasksStatus: ChangeTasksStatusRequest;
	registerExtension: RegisterExtensionRequest;
	/**
	 * Removes extension `extension_id`, stopping its sandbox and timers and
	 * dropping its permission and hook grants.
	 */
	uninstallExtension: UninstallExtensionRequest;
	/** Lists every installed extension, in the order its hooks run. */
	listExtensions: NoArgs;
	/** Lists the permissions recorded for extension `extension_id`. */
	listExtensionPermissions: ListExtensionPermissionsRequest;
	/**
	 * Enables or disables `permission` for extension `extension_id`,
	 * restarting its sandbox so the change applies at once.
	 */
	setExtensionPermission: SetExtensionPermissionRequest;
	/** Lists the hooks recorded for extension `extension_id`. */
	listExtensionHooks: ListExtensionHooksRequest;
	/**
	 * Enables or disables `hook` for extension `extension_id`, restarting
	 * its sandbox; enabling `launch` calls `onLaunch`.
	 */
	setExtensionHook: SetExtensionHookRequest;
	/**
	 * Asks every extension granted `registry` for the extensions it offers
	 * matching `query`, one section per registry.
	 */
	searchExtensionRegistries: SearchExtensionRegistriesRequest;
	/**
	 * Lists the updates the last check found for installed extensions: one
	 * per extension whose version a registry lists differently.
	 */
	listExtensionUpdates: NoArgs;
	/**
	 * Asks every registry about the installed extensions now, and lists the
	 * updates found.
	 */
	checkExtensionUpdates: NoArgs;
	/**
	 * Downloads an extension script and reads its manifest, for review
	 * before installing it.
	 */
	previewExtension: PreviewExtensionRequest;
	/**
	 * Reads the manifest of an extension script, for review before
	 * installing it.
	 */
	previewExtensionScript: PreviewExtensionScriptRequest;
	/**
	 * Calls extension `extension_id`'s `serviceRequest(name, args, caller)`,
	 * which needs its `service` permission, telling it which client asks,
	 * and answers what it returns.
	 */
	callExtension: CallExtensionRequest;
	/**
	 * Lists every client that is not deleted, each saying whether it is
	 * connected now and whether it is the one asking.
	 */
	listClients: NoArgs;
	/**
	 * Revokes client `id`; its certificate is no longer trusted, and its
	 * session no longer serves its actors.
	 */
	deleteClient: DeleteClientRequest;
	/** Renames client `id`; its name is only how it is listed. */
	renameClient: RenameClientRequest;
	/** Revokes every client but the one sending this, and answers them. */
	revokeOtherClients: NoArgs;
	/** Answers the security settings. */
	getSecuritySettings: NoArgs;
	/** Replaces the security settings. */
	updateSecuritySettings: UpdateSecuritySettingsRequest;
	/** Lists a page of the security activity, newest first. */
	listSecurityEvents: ListSecurityEventsRequest;
	/**
	 * Answers the storage settings: how long the daemon keeps model
	 * threads, actor sessions and security activity, and how many memories
	 * each scope keeps.
	 */
	getStorageSettings: NoArgs;
	/** Replaces the storage settings. */
	updateStorageSettings: UpdateStorageSettingsRequest;
	/** Answers how large the database file is. */
	getDatabaseSize: NoArgs;
	/** Writes a backup of the database to a new file on the daemon's computer. */
	backupDatabase: NoArgs;
	/**
	 * Copies the whole database to a new file on the daemon's computer, for
	 * another daemon to import; read it with `ReadDatabaseExport`.
	 */
	exportDatabase: NoArgs;
	/** Answers part of an export, removing it once the part reaches the end. */
	readDatabaseExport: ReadDatabaseExportRequest;
	/** Starts an import, which `WriteDatabaseImport` fills. */
	beginDatabaseImport: NoArgs;
	/** Adds part of the file to an import. */
	writeDatabaseImport: WriteDatabaseImportRequest;
	/**
	 * Replaces everything in the database with an import, keeping this
	 * server's key and clients, then restarts the daemon.
	 */
	finishDatabaseImport: FinishDatabaseImportRequest;
	/** Answers the user's profile. */
	getProfile: NoArgs;
	/** Sets or removes the user's name. */
	setProfileName: SetProfileNameRequest;
	/** Sets the user's bio, or removes it when it is `null` or empty. */
	setProfileBio: SetProfileBioRequest;
	/** Sets or removes the user's photo. */
	setProfilePhoto: SetProfilePhotoRequest;
	/** Creates an agent. */
	createIntelligenceAgent: CreateIntelligenceAgentRequest;
	/**
	 * Lists every agent: the pinned ones in their order, then the rest by
	 * latest message, newest first.
	 */
	listIntelligenceAgents: NoArgs;
	/** Reorders the pinned agents; replies with every agent in the new order. */
	reorderIntelligenceAgents: ReorderIntelligenceAgentsRequest;
	/** Pins or unpins agent `agent_id`. */
	setAgentPinned: SetAgentPinnedRequest;
	/** Puts agent `agent_id` on a team, or on none. */
	setAgentTeam: SetAgentTeamRequest;
	/** Lists every team of agents, by title. */
	listAgentTeams: NoArgs;
	/** Creates a team of agents. */
	createAgentTeam: CreateAgentTeamRequest;
	/** Renames team `team_id`. */
	renameAgentTeam: RenameAgentTeamRequest;
	/**
	 * Deletes team `team_id`; its agents stay, on no team, and its
	 * memories are forgotten.
	 */
	deleteAgentTeam: DeleteAgentTeamRequest;
	/** Fetches agent `agent_id`. */
	getIntelligenceAgent: GetIntelligenceAgentRequest;
	/** Renames, recolors, sets the emoji of or changes the model of agent `agent_id`. */
	updateIntelligenceAgent: UpdateIntelligenceAgentRequest;
	/** Deletes agent `agent_id` with its cues, runs and messages. */
	deleteIntelligenceAgent: DeleteIntelligenceAgentRequest;
	/** Marks agent `agent_id`'s conversation read. */
	markAgentRead: MarkAgentReadRequest;
	/** Pages through an agent's messages, oldest first. */
	listAgentMessages: ListAgentMessagesRequest;
	/** Fetches agent message `message_id`. */
	getAgentMessage: GetAgentMessageRequest;
	/**
	 * Reacts to an agent's message with an emoji, or clears the reaction;
	 * a reaction queues the agent's turn on it.
	 */
	setAgentMessageReaction: SetAgentMessageReactionRequest;
	/** Lists the images attached to a message. */
	listAgentMessageAttachments: ListAgentMessageAttachmentsRequest;
	/**
	 * Sends a message to an agent: a running turn takes it in, or it
	 * queues the agent's turn on it.
	 */
	sendAgentMessage: SendAgentMessageRequest;
	/** Stops the running turn of an agent's conversation. */
	stopAgentTurn: StopAgentTurnRequest;
	/** Picks an option of an agent's choice, queueing its turn on the pick. */
	respondToAgentChoice: RespondToAgentChoiceRequest;
	/** Lists agent `agent_id`'s cues, oldest first. */
	listAgentCues: ListAgentCuesRequest;
	/** Removes cue `cue_id`; an agent left with no cues goes idle. */
	removeAgentCue: RemoveAgentCueRequest;
	/**
	 * Lists the memories agent `agent_id` sees, its own and the global
	 * ones; only the global ones when absent. Oldest first.
	 */
	listMemories: ListMemoriesRequest;
	/** Forgets memory `memory_id`, returning it. */
	forgetMemory: ForgetMemoryRequest;
	/** Lists agent `agent_id`'s most recent background runs, newest first. */
	listAgentRuns: ListAgentRunsRequest;
	/**
	 * Lists the activities still running or waiting, then the `limit`
	 * that ended most recently.
	 */
	listActivities: ListActivitiesRequest;
	/** Lists the actors the clients offer. */
	listActors: NoArgs;
	/**
	 * Lists how much of its limits each actor whose client is connected
	 * has used.
	 */
	listActorUsage: NoArgs;
	/** Lists actor sessions, latest status change first. */
	listActorSessions: ListActorSessionsRequest;
	getActorSession: GetActorSessionRequest;
	/**
	 * Lists an actor session's subagents, one level down, first launched
	 * first.
	 */
	listActorSubagents: ListActorSubagentsRequest;
	/**
	 * Lists an actor session's family: the session Cohesion started, then
	 * every subagent beneath it, each after the one it runs under.
	 */
	listActorSessionFamily: ListActorSessionFamilyRequest;
	/**
	 * Lists the requests waiting on the user in an actor session's
	 * subagents, oldest first.
	 */
	listUnansweredSubagentRequests: ListUnansweredSubagentRequestsRequest;
	/**
	 * Full-text matches among actor sessions' titles and transcripts, best
	 * first.
	 */
	searchActorSessions: SearchActorSessionsRequest;
	/** Marks actor session `session_id` read. */
	markActorSessionRead: MarkActorSessionReadRequest;
	/** Lists what was said in an actor session, oldest first. */
	listActorSessionMessages: ListActorSessionMessagesRequest;
	/** Lists the images attached to an actor session message. */
	listActorSessionMessageAttachments: ListActorSessionMessageAttachmentsRequest;
	/** Hands an actor a new session, through its client. */
	startActorSession: StartActorSessionRequest;
	/** Says something in an actor session, through its client. */
	sendActorSessionMessage: SendActorSessionMessageRequest;
	/** Stops what an actor session is doing, leaving it idle. */
	stopActorSession: StopActorSessionRequest;
	/** Resolves an actor session, stopping it first if it is going on. */
	resolveActorSession: ResolveActorSessionRequest;
	/** Switches an actor session's mode, through its client. */
	setActorSessionMode: SetActorSessionModeRequest;
	/** Lists every service a `connector`-granted extension reaches. */
	listConnectorServices: NoArgs;
	/** Asks connector extension `extension_id` for its sign-in page. */
	beginConnectorAuthentication: BeginConnectorAuthenticationRequest;
	/** Adds (or re-authenticates) the account a sign-in `code` belongs to. */
	completeConnectorAuthentication: CompleteConnectorAuthenticationRequest;
	/** Lists every connector account. */
	listConnectorAccounts: NoArgs;
	/**
	 * Removes connector account `account_id` and every resource imported
	 * from it, then tells its extension with `accountRemoved`.
	 */
	deleteConnectorAccount: DeleteConnectorAccountRequest;
	/**
	 * Moves every resource of one connector account to another of the
	 * same connector.
	 */
	migrateResources: MigrateResourcesRequest;
	/** Lists the organization units of connector account `account_id`. */
	listConnectorOrganizationUnits: ListConnectorOrganizationUnitsRequest;
	/** Lists the items a connector account offers for import into a workspace. */
	searchConnectorResources: SearchConnectorResourcesRequest;
	/** Imports one item of a connector account into a workspace as a resource. */
	importResource: ImportResourceRequest;
	/** Lists the resources in a workspace, most recently updated first. */
	listResources: ListResourcesRequest;
	/** Fetches resource `resource_id` in a workspace. */
	getResource: GetResourceRequest;
	/**
	 * Asks the connector of resource `resource_id` for the interface it
	 * shows the resource with, from its `renderResource`; `null` when it
	 * has none, so the client shows the contents.
	 */
	renderResource: RenderResourceRequest;
	/** Removes resource `resource_id` from a workspace. */
	deleteResource: DeleteResourceRequest;
	/** Refreshes every resource in a workspace from its service now. */
	refreshResources: RefreshResourcesRequest;
	/** Lists the kinds of resource shown in a workspace. */
	listWorkspaceResourceKinds: ListWorkspaceResourceKindsRequest;
	/** Shows or hides a kind of resource in a workspace. */
	setResourceKindShown: SetResourceKindShownRequest;
	/** Reorders the kinds of resource shown in a workspace. */
	reorderWorkspaceResourceKinds: ReorderWorkspaceResourceKindsRequest;
	/**
	 * Lists a page of the live articles from a feed, or from every feed, most
	 * recently published first, without their contents.
	 */
	listArticles: ListArticlesRequest;
	/** Marks live article `article_id` read or unread. */
	setArticleRead: SetArticleReadRequest;
	/** Marks every unread article a listing covers as read. */
	markArticlesRead: MarkArticlesReadRequest;
	/** Lists the feeds in their order, with their unread counts. */
	listFeeds: NoArgs;
	/** Reads a feed and follows it. */
	addFeed: AddFeedRequest;
	/** Reads live feed `feed_id` now. */
	refreshFeed: RefreshFeedRequest;
	/** Reads every live feed now. */
	refreshFeeds: NoArgs;
	/** Retitles live feed `feed_id`. */
	renameFeed: RenameFeedRequest;
	/** Reorders the feeds; replies with them in their new order. */
	reorderFeeds: ReorderFeedsRequest;
	/** Sets when live feed `feed_id`'s items are read from their pages. */
	setFeedFullPagePolicy: SetFeedFullPagePolicyRequest;
	/** Stops following feed `feed_id`, removing its items. */
	deleteFeed: DeleteFeedRequest;
	/** Removes feeds' items published before a time. */
	pruneFeeds: PruneFeedsRequest;
	/** A page of the full-text matches among the articles from feeds. */
	searchFeedCorpus: SearchFeedCorpusRequest;
	/** Fetches live article `article_id`, whole. */
	getArticle: GetArticleRequest;
	/** Fetches live article `article_id`'s page again. */
	resnapshotArticle: ResnapshotArticleRequest;
	/** Retitles live article `article_id`. */
	renameArticle: RenameArticleRequest;
}

/** The `ok.data` each request type answers with. */
export interface Replies {
	about: AboutResponse;
	/** Answers the web UI this cohesiond was built with, for a client to serve. */
	webBundle: WebBundle;
	/**
	 * Answers the hash of the web UI this cohesiond was built with, so a
	 * client can check its copy without fetching the files.
	 */
	webBundleHash: WebBundleHash;
	/**
	 * Opens a pairing window and answers the setup code a new client
	 * registers with. It works for one registration, for ten minutes.
	 * Refused to a client the security settings don't let open pairing.
	 */
	beginPairing: PairingCode;
	/** Answers whether the client asking may open pairing now, and why not. */
	getPairingAccess: PairingAccess;
	/** Evaluates a script on the server. */
	runScript: ScriptResult;
	/** Draws a Mermaid diagram, or says why it can't be drawn. */
	renderMermaid: RenderedMermaid;
	/** Lists every workspace that is not deleted. */
	listWorkspaces: Workspace[];
	/** Creates a workspace with `title`. */
	createWorkspace: Workspace;
	/** Deletes workspace `id`. */
	deleteWorkspace: Workspace;
	/** Retitles workspace `id`. */
	renameWorkspace: Workspace;
	/** Sets or clears a workspace's emoji. */
	setWorkspaceEmoji: Workspace;
	/** Fetches the journal for a date in a workspace, if it exists. */
	getJournal: Document | null;
	/** Appends `content` to the journal for a date in a workspace. */
	appendToJournal: Document;
	/** Replaces the contents of the journal for a date in a workspace. */
	editJournal: Document;
	/** Search documents */
	searchDocuments: Document[];
	/** Lists the live documents in a workspace, most recently modified first. */
	listDocuments: Document[];
	/** Fetches live document `document_id` in a workspace. */
	getDocument: Document;
	/** Replaces the contents of live document `document_id` in a workspace. */
	editDocument: Document;
	/** Creates an ordinary document in a workspace. */
	createDocument: Document;
	/** Retitles live document `document_id` in a workspace. */
	renameDocument: Document;
	/** Deletes document `document_id` in a workspace. */
	deleteDocument: Document;
	/** Files a document into a folder, or the Inbox. */
	moveDocument: Document;
	/** Lists a workspace's document folders. */
	listDocumentFolders: DocumentFolder[];
	/** Creates a document folder, at the top level or inside another. */
	createDocumentFolder: DocumentFolder;
	/** Retitles a document folder. */
	renameDocumentFolder: DocumentFolder;
	/**
	 * Moves a document folder into another, or the top level, and among its
	 * new siblings; replies with every folder in the workspace.
	 */
	moveDocumentFolder: DocumentFolder[];
	/** Deletes a document folder and the folders inside it. */
	deleteDocumentFolder: DocumentFolder;
	/** Lists an item's kept versions, newest first, without their text. */
	listVersions: VersionSummary[];
	/** Fetches one kept version. */
	getVersion: Version;
	/**
	 * Puts an item back to a kept version, bringing it back if it was
	 * deleted, and keeps the result as a new version.
	 */
	restoreVersion: RestoredItem;
	/** Lists the deleted items Rewind can still bring back. */
	listDeletedItems: DeletedItem[];
	/** Lists task folders */
	listTaskFolders: TaskFolder[];
	createTaskFolder: TaskFolder;
	deleteTaskFolder: TaskFolder;
	/** Retitles task folder `folder_id` in a workspace. */
	renameTaskFolder: TaskFolder;
	/** Reorders a workspace's task folders; replies with them in their new order. */
	reorderTaskFolders: TaskFolder[];
	/** Lists a workspace's saved task views, in their order. */
	listSavedTaskViews: SavedTaskView[];
	/** Saves a way of looking at a workspace's tasks under a name. */
	createSavedTaskView: SavedTaskView;
	/** Replaces what saved task view `view_id` lists, and in what order. */
	updateSavedTaskView: SavedTaskView;
	/** Renames saved task view `view_id` in a workspace. */
	renameSavedTaskView: SavedTaskView;
	/** Reorders a workspace's saved task views; replies with them in their new order. */
	reorderSavedTaskViews: SavedTaskView[];
	/** Deletes saved task view `view_id` in a workspace. */
	deleteSavedTaskView: SavedTaskView;
	/** Lists a workspace's tags, in their order. */
	listTags: Tag[];
	/** Creates a tag in a workspace. */
	createTag: Tag;
	/** Renames tag `tag_id` in a workspace. */
	renameTag: Tag;
	/** Reorders a workspace's tags; replies with them in their new order. */
	reorderTags: Tag[];
	/** Deletes tag `tag_id` in a workspace, taking it off everything. */
	deleteTag: Tag;
	/** Puts a tag on a task or takes it off; replies with the task. */
	setTagged: Task;
	/** Lists a workspace's live contacts, by last name, then first name. */
	listContacts: Contact[];
	/** Fetches live contact `contact_id` in a workspace. */
	getContact: Contact;
	/** Creates a contact in a workspace. */
	createContact: Contact;
	/** Replaces contact `contact_id`'s names and notes. */
	updateContact: Contact;
	/** Deletes contact `contact_id` in a workspace. */
	deleteContact: Contact;
	/** Adds an email address or phone number to contact `contact_id`. */
	addContactMethod: ContactMethod;
	/** Replaces contact method `method_id`'s kind and value. */
	updateContactMethod: ContactMethod;
	/** Removes contact method `method_id` from its contact. */
	deleteContactMethod: ContactMethod;
	/** Sets or removes contact `contact_id`'s photo. */
	setContactPhoto: Contact;
	/** Lists a page of the live tasks the filters cover. */
	listTasks: TaskPage;
	createTask: Task;
	/** Retitles task `task_id` in a workspace. */
	renameTask: Task;
	/** Sets the priority of task `task_id` in a workspace. */
	setTaskPriority: Task;
	/** Sets the size of task `task_id` in a workspace; `null` clears it. */
	setTaskSize: Task;
	/**
	 * Assigns task `task_id` in a workspace to the user or one of their
	 * agents; `null` leaves it unassigned.
	 */
	setTaskAssignee: Task;
	/** Replaces the description of task `task_id` in a workspace; `""` clears it. */
	editTaskDescription: Task;
	/** Sets both dates of task `task_id` in a workspace; `null` clears one. */
	scheduleTask: Task;
	/** Moves task `task_id` into folder `folder_id`, or the inbox when absent. */
	moveTask: Task;
	/** Deletes task `task_id` in a workspace. */
	deleteTask: Task;
	searchWorkspace: SearchResult[];
	documentation: DocumentationResponse;
	updateModelCatalog: AcceptedResponse;
	/** Lists every intelligence endpoint. */
	listIntelligenceEndpoints: RedactedIntelligenceEndpoint[];
	createIntelligenceEndpoint: RedactedIntelligenceEndpoint;
	deleteIntelligenceEndpoint: RedactedIntelligenceEndpoint;
	/** Sets or removes where a person sees what an endpoint has used. */
	setIntelligenceEndpointUsageUrl: RedactedIntelligenceEndpoint;
	/** Sets how long an endpoint's server keeps a prompt cached. */
	setIntelligenceEndpointCacheMinutes: RedactedIntelligenceEndpoint;
	/** Sets or removes the longer prompt cache an endpoint's server offers. */
	setIntelligenceEndpointLongCache: RedactedIntelligenceEndpoint;
	/** Imports a model into an endpoint, from the catalog when it lists it. */
	addEndpointModel: EndpointModel;
	/** Removes an endpoint's model, unsetting any tier that uses it. */
	removeEndpointModel: EndpointModel;
	/** Lists the models an endpoint offers. */
	listEndpointModels: EndpointModel[];
	/** Makes an endpoint model the default for a tier. */
	setIntelligenceDefault: AcceptedResponse;
	listIntelligenceDefaults: IntelligenceDefault[];
	/** Removes the default model of `tier`. */
	clearIntelligenceDefault: AcceptedResponse;
	/** Lists what model calls cost over each spend period, against its limit. */
	listIntelligenceSpend: IntelligenceSpend[];
	/** Sets the spend limit of a period, answering every period's spend. */
	setIntelligenceSpendLimit: IntelligenceSpend[];
	/** Removes the spend limit of a period, answering every period's spend. */
	clearIntelligenceSpendLimit: IntelligenceSpend[];
	listCatalogModels: CatalogModel[];
	/** Fetches intelligence thread `thread_id` with its messages. */
	getIntelligenceThread: IntelligenceThreadDetail;
	/** Lists the `limit` most recent intelligence cost events, newest first. */
	listIntelligenceCostEvents: IntelligenceCostEvent[];
	changeTasksStatus: Task[];
	registerExtension: AcceptedResponse;
	/**
	 * Removes extension `extension_id`, stopping its sandbox and timers and
	 * dropping its permission and hook grants.
	 */
	uninstallExtension: AcceptedResponse;
	/** Lists every installed extension, in the order its hooks run. */
	listExtensions: ExtensionSummary[];
	/** Lists the permissions recorded for extension `extension_id`. */
	listExtensionPermissions: ExtensionPermission[];
	/**
	 * Enables or disables `permission` for extension `extension_id`,
	 * restarting its sandbox so the change applies at once.
	 */
	setExtensionPermission: AcceptedResponse;
	/** Lists the hooks recorded for extension `extension_id`. */
	listExtensionHooks: ExtensionHook[];
	/**
	 * Enables or disables `hook` for extension `extension_id`, restarting
	 * its sandbox; enabling `launch` calls `onLaunch`.
	 */
	setExtensionHook: AcceptedResponse;
	/**
	 * Asks every extension granted `registry` for the extensions it offers
	 * matching `query`, one section per registry.
	 */
	searchExtensionRegistries: RegistrySection[];
	/**
	 * Lists the updates the last check found for installed extensions: one
	 * per extension whose version a registry lists differently.
	 */
	listExtensionUpdates: ExtensionUpdate[];
	/**
	 * Asks every registry about the installed extensions now, and lists the
	 * updates found.
	 */
	checkExtensionUpdates: ExtensionUpdate[];
	/**
	 * Downloads an extension script and reads its manifest, for review
	 * before installing it.
	 */
	previewExtension: ExtensionPreview;
	/**
	 * Reads the manifest of an extension script, for review before
	 * installing it.
	 */
	previewExtensionScript: ExtensionPreview;
	/**
	 * Calls extension `extension_id`'s `serviceRequest(name, args, caller)`,
	 * which needs its `service` permission, telling it which client asks,
	 * and answers what it returns.
	 */
	callExtension: ExtensionCallResult;
	/**
	 * Lists every client that is not deleted, each saying whether it is
	 * connected now and whether it is the one asking.
	 */
	listClients: ListedClient[];
	/**
	 * Revokes client `id`; its certificate is no longer trusted, and its
	 * session no longer serves its actors.
	 */
	deleteClient: Client;
	/** Renames client `id`; its name is only how it is listed. */
	renameClient: Client;
	/** Revokes every client but the one sending this, and answers them. */
	revokeOtherClients: Client[];
	/** Answers the security settings. */
	getSecuritySettings: SecuritySettings;
	/** Replaces the security settings. */
	updateSecuritySettings: SecuritySettings;
	/** Lists a page of the security activity, newest first. */
	listSecurityEvents: SecurityEventPage;
	/**
	 * Answers the storage settings: how long the daemon keeps model
	 * threads, actor sessions and security activity, and how many memories
	 * each scope keeps.
	 */
	getStorageSettings: StorageSettings;
	/** Replaces the storage settings. */
	updateStorageSettings: StorageSettings;
	/** Answers how large the database file is. */
	getDatabaseSize: DatabaseSize;
	/** Writes a backup of the database to a new file on the daemon's computer. */
	backupDatabase: DatabaseBackup;
	/**
	 * Copies the whole database to a new file on the daemon's computer, for
	 * another daemon to import; read it with `ReadDatabaseExport`.
	 */
	exportDatabase: DatabaseExport;
	/** Answers part of an export, removing it once the part reaches the end. */
	readDatabaseExport: DatabaseExportPart;
	/** Starts an import, which `WriteDatabaseImport` fills. */
	beginDatabaseImport: DatabaseImport;
	/** Adds part of the file to an import. */
	writeDatabaseImport: AcceptedResponse;
	/**
	 * Replaces everything in the database with an import, keeping this
	 * server's key and clients, then restarts the daemon.
	 */
	finishDatabaseImport: AcceptedResponse;
	/** Answers the user's profile. */
	getProfile: Profile;
	/** Sets or removes the user's name. */
	setProfileName: Profile;
	/** Sets the user's bio, or removes it when it is `null` or empty. */
	setProfileBio: Profile;
	/** Sets or removes the user's photo. */
	setProfilePhoto: Profile;
	/** Creates an agent. */
	createIntelligenceAgent: IntelligenceAgent;
	/**
	 * Lists every agent: the pinned ones in their order, then the rest by
	 * latest message, newest first.
	 */
	listIntelligenceAgents: IntelligenceAgent[];
	/** Reorders the pinned agents; replies with every agent in the new order. */
	reorderIntelligenceAgents: IntelligenceAgent[];
	/** Pins or unpins agent `agent_id`. */
	setAgentPinned: IntelligenceAgent;
	/** Puts agent `agent_id` on a team, or on none. */
	setAgentTeam: IntelligenceAgent;
	/** Lists every team of agents, by title. */
	listAgentTeams: IntelligenceAgentTeam[];
	/** Creates a team of agents. */
	createAgentTeam: IntelligenceAgentTeam;
	/** Renames team `team_id`. */
	renameAgentTeam: IntelligenceAgentTeam;
	/**
	 * Deletes team `team_id`; its agents stay, on no team, and its
	 * memories are forgotten.
	 */
	deleteAgentTeam: IntelligenceAgentTeam;
	/** Fetches agent `agent_id`. */
	getIntelligenceAgent: IntelligenceAgent;
	/** Renames, recolors, sets the emoji of or changes the model of agent `agent_id`. */
	updateIntelligenceAgent: IntelligenceAgent;
	/** Deletes agent `agent_id` with its cues, runs and messages. */
	deleteIntelligenceAgent: IntelligenceAgent;
	/** Marks agent `agent_id`'s conversation read. */
	markAgentRead: IntelligenceAgent;
	/** Pages through an agent's messages, oldest first. */
	listAgentMessages: IntelligenceAgentMessage[];
	/** Fetches agent message `message_id`. */
	getAgentMessage: IntelligenceAgentMessage;
	/**
	 * Reacts to an agent's message with an emoji, or clears the reaction;
	 * a reaction queues the agent's turn on it.
	 */
	setAgentMessageReaction: AcceptedResponse;
	/** Lists the images attached to a message. */
	listAgentMessageAttachments: Attachment[];
	/**
	 * Sends a message to an agent: a running turn takes it in, or it
	 * queues the agent's turn on it.
	 */
	sendAgentMessage: AcceptedResponse;
	/** Stops the running turn of an agent's conversation. */
	stopAgentTurn: AcceptedResponse;
	/** Picks an option of an agent's choice, queueing its turn on the pick. */
	respondToAgentChoice: AcceptedResponse;
	/** Lists agent `agent_id`'s cues, oldest first. */
	listAgentCues: IntelligenceAgentCue[];
	/** Removes cue `cue_id`; an agent left with no cues goes idle. */
	removeAgentCue: AcceptedResponse;
	/**
	 * Lists the memories agent `agent_id` sees, its own and the global
	 * ones; only the global ones when absent. Oldest first.
	 */
	listMemories: Memory[];
	/** Forgets memory `memory_id`, returning it. */
	forgetMemory: Memory;
	/** Lists agent `agent_id`'s most recent background runs, newest first. */
	listAgentRuns: IntelligenceAgentRun[];
	/**
	 * Lists the activities still running or waiting, then the `limit`
	 * that ended most recently.
	 */
	listActivities: Activity[];
	/** Lists the actors the clients offer. */
	listActors: Actor[];
	/**
	 * Lists how much of its limits each actor whose client is connected
	 * has used.
	 */
	listActorUsage: ListedActorUsage[];
	/** Lists actor sessions, latest status change first. */
	listActorSessions: ActorSession[];
	getActorSession: ActorSession;
	/**
	 * Lists an actor session's subagents, one level down, first launched
	 * first.
	 */
	listActorSubagents: ActorSession[];
	/**
	 * Lists an actor session's family: the session Cohesion started, then
	 * every subagent beneath it, each after the one it runs under.
	 */
	listActorSessionFamily: ActorSession[];
	/**
	 * Lists the requests waiting on the user in an actor session's
	 * subagents, oldest first.
	 */
	listUnansweredSubagentRequests: ActorSessionMessage[];
	/**
	 * Full-text matches among actor sessions' titles and transcripts, best
	 * first.
	 */
	searchActorSessions: ActorSessionSearchHit[];
	/** Marks actor session `session_id` read. */
	markActorSessionRead: ActorSession;
	/** Lists what was said in an actor session, oldest first. */
	listActorSessionMessages: ActorSessionMessage[];
	/** Lists the images attached to an actor session message. */
	listActorSessionMessageAttachments: Attachment[];
	/** Hands an actor a new session, through its client. */
	startActorSession: ActorSession;
	/** Says something in an actor session, through its client. */
	sendActorSessionMessage: ActorSessionMessage;
	/** Stops what an actor session is doing, leaving it idle. */
	stopActorSession: ActorSession;
	/** Resolves an actor session, stopping it first if it is going on. */
	resolveActorSession: ActorSession;
	/** Switches an actor session's mode, through its client. */
	setActorSessionMode: ActorSession;
	/** Lists every service a `connector`-granted extension reaches. */
	listConnectorServices: ConnectorService[];
	/** Asks connector extension `extension_id` for its sign-in page. */
	beginConnectorAuthentication: ConnectorAuthentication;
	/** Adds (or re-authenticates) the account a sign-in `code` belongs to. */
	completeConnectorAuthentication: RedactedConnectorAccount;
	/** Lists every connector account. */
	listConnectorAccounts: RedactedConnectorAccount[];
	/**
	 * Removes connector account `account_id` and every resource imported
	 * from it, then tells its extension with `accountRemoved`.
	 */
	deleteConnectorAccount: RedactedConnectorAccount;
	/**
	 * Moves every resource of one connector account to another of the
	 * same connector.
	 */
	migrateResources: AcceptedResponse;
	/** Lists the organization units of connector account `account_id`. */
	listConnectorOrganizationUnits: OrganizationUnit[];
	/** Lists the items a connector account offers for import into a workspace. */
	searchConnectorResources: ImportableResource[];
	/** Imports one item of a connector account into a workspace as a resource. */
	importResource: Resource;
	/** Lists the resources in a workspace, most recently updated first. */
	listResources: Resource[];
	/** Fetches resource `resource_id` in a workspace. */
	getResource: Resource;
	/**
	 * Asks the connector of resource `resource_id` for the interface it
	 * shows the resource with, from its `renderResource`; `null` when it
	 * has none, so the client shows the contents.
	 */
	renderResource: Component | null;
	/** Removes resource `resource_id` from a workspace. */
	deleteResource: Resource;
	/** Refreshes every resource in a workspace from its service now. */
	refreshResources: ResourceRefreshSummary;
	/** Lists the kinds of resource shown in a workspace. */
	listWorkspaceResourceKinds: WorkspaceResourceKind[];
	/** Shows or hides a kind of resource in a workspace. */
	setResourceKindShown: WorkspaceResourceKind[];
	/** Reorders the kinds of resource shown in a workspace. */
	reorderWorkspaceResourceKinds: WorkspaceResourceKind[];
	/**
	 * Lists a page of the live articles from a feed, or from every feed, most
	 * recently published first, without their contents.
	 */
	listArticles: ArticlePage;
	/** Marks live article `article_id` read or unread. */
	setArticleRead: Article;
	/** Marks every unread article a listing covers as read. */
	markArticlesRead: ArticlesMarkedRead;
	/** Lists the feeds in their order, with their unread counts. */
	listFeeds: FeedSummary[];
	/** Reads a feed and follows it. */
	addFeed: Feed;
	/** Reads live feed `feed_id` now. */
	refreshFeed: Feed;
	/** Reads every live feed now. */
	refreshFeeds: Feed[];
	/** Retitles live feed `feed_id`. */
	renameFeed: Feed;
	/** Reorders the feeds; replies with them in their new order. */
	reorderFeeds: FeedSummary[];
	/** Sets when live feed `feed_id`'s items are read from their pages. */
	setFeedFullPagePolicy: Feed;
	/** Stops following feed `feed_id`, removing its items. */
	deleteFeed: Feed;
	/** Removes feeds' items published before a time. */
	pruneFeeds: FeedCleanup;
	/** A page of the full-text matches among the articles from feeds. */
	searchFeedCorpus: FeedCorpusPage;
	/** Fetches live article `article_id`, whole. */
	getArticle: Article;
	/** Fetches live article `article_id`'s page again. */
	resnapshotArticle: RefetchedArticle;
	/** Retitles live article `article_id`. */
	renameArticle: Article;
}
