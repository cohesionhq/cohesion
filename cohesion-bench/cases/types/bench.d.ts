// The bench's own globals, beside Cohesion's script API in cohesion.d.ts.
// `B`, the helpers every case shares, is declared by prelude.ts itself.

/** What the agent left that `validate.ts` reads besides the database. */
interface Left {
  /** What it told the user: its messages and questions, joined by blank lines. */
  reply: string;
  /** The cues it holds once its turn has ended. */
  cues: CueLeft[];
  /** The memories it holds once its turn has ended. */
  memories: string[];
}

interface CueLeft {
  name: string;
  /** Such as `"task:create"` or `"timer"`. */
  hook: string;
  /** The cue's function, as source text. */
  code: string;
  instructions: string;
  effort: string;
}

/** In `validate.ts`: what the agent left. */
declare const bench: Left;
/** In `validate.ts`: `bench.reply`. */
declare const reply: string;

/**
 * A starting state, as `states/*.ts` exports it: plain data, read into
 * `StartingState` in src/starting_state.rs, which refuses fields it
 * doesn't know. Days count from today: -1 is yesterday.
 */
interface StartingState {
  workspaces: WorkspaceState[];
}

interface WorkspaceState {
  title: string;
  tags?: string[];
  /** Task folders, by title. */
  taskFolders?: string[];
  contacts?: ContactState[];
  tasks?: TaskState[];
  /** Top-level document folders, with what they hold. */
  documentFolders?: DocumentFolderState[];
  /** Documents in the Inbox. */
  documents?: DocumentState[];
  journals?: JournalState[];
}

interface ContactState {
  firstName: string;
  lastName: string;
  methods?: { kind: ContactMethodKind; value: string }[];
  notes?: string;
}

interface TaskState {
  title: string;
  /** Its task folder, by title. */
  folder?: string;
  priority?: TaskPriority;
  size?: TaskSize;
  /** Days from today. */
  start?: number;
  /** Days from today. */
  due?: number;
  description?: string;
  /** Tag names, from the workspace's tags. */
  tags?: string[];
  status?: TaskStatus;
  assignee?: { kind: 'user' } | { kind: 'contact'; firstName: string; lastName: string };
}

interface DocumentFolderState {
  title: string;
  folders?: DocumentFolderState[];
  documents?: DocumentState[];
}

interface DocumentState {
  title: string;
  contents: string;
}

interface JournalState {
  /** Days from today. */
  day: number;
  contents: string;
}
