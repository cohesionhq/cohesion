// Run ahead of every case's setup, validate and solution, and of every
// starting state. `B` holds what they share; nothing else here is global.
// A script, not a module, so `B` is global in an editor too.

const B = (() => {
  const pad = (n: number) => String(n).padStart(2, '0');
  const iso = (date: Date) => `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  /** The local date `offset` days from today, at noon. */
  const date = (offset = 0) => {
    const at = new Date();
    at.setHours(12, 0, 0, 0);
    at.setDate(at.getDate() + offset);
    return at;
  };
  /** The local date `offset` days from today, as `YYYY-MM-DD`. */
  const day = (offset = 0) => iso(date(offset));
  const ALL: TaskStatus[] = ['incomplete', 'completed', 'cancelled'];
  /**
   * In place of a workspace's title: every workspace. For something the
   * prompt doesn't say where to put, where the agent put it is its call.
   */
  const ANY = '*';
  const ws = (title = 'Personal') => cohesion.workspace(title);
  /** The workspace titled `title`, or every workspace for `ANY`. */
  const each = (title: string) => (title === ANY ? cohesion.workspaces() : [ws(title)]);
  /** Every task of a workspace, in any status. */
  const tasks = (title = 'Personal', options: TaskListOptions = {}) =>
    each(title).flatMap((w) => w.tasks({ status: ALL, ...options }));
  /** The one task with exactly this title, in any status; throws for none or several. */
  const task = (name: string, title = 'Personal') => {
    const found = tasks(title).filter((t) => t.title === name);
    if (found.length !== 1) throw new Error(`${found.length} tasks titled "${name}"`);
    return found[0];
  };
  /** Tasks whose title matches `pattern`, in any status. */
  const tasksLike = (pattern: RegExp, title = 'Personal') => tasks(title).filter((t) => pattern.test(t.title));
  /** The one document with exactly this title; throws for none or several. */
  const doc = (name: string, title = 'Personal') => {
    const found = ws(title).documents().filter((d) => d.title === name);
    if (found.length !== 1) throw new Error(`${found.length} documents titled "${name}"`);
    return found[0];
  };
  /** Documents whose title matches `pattern`. */
  const docsLike = (pattern: RegExp, title = 'Personal') =>
    each(title).flatMap((w) => w.documents()).filter((d) => pattern.test(d.title));
  const one = <T>(found: T | undefined, what: string): T => {
    if (found === undefined) throw new Error(`no ${what}`);
    return found;
  };
  /** The task folder with exactly this title; throws when there is none. */
  const taskFolder = (name: string, title = 'Personal') =>
    one(ws(title).taskFolders().find((f) => f.title === name), `task folder "${name}" in ${title}`);
  /** The task folder with exactly this title, or `null`. */
  const findTaskFolder = (name: string, title = 'Personal') =>
    each(title).flatMap((w) => w.taskFolders()).find((f) => f.title === name) ?? null;
  /** The document folder with exactly this title; throws when there is none. */
  const docFolder = (name: string, title = 'Personal') =>
    one(ws(title).documentFolders().find((f) => f.title === name), `document folder "${name}" in ${title}`);
  /** The document folder with exactly this title, or `null`. */
  const findDocFolder = (name: string, title = 'Personal') =>
    each(title).flatMap((w) => w.documentFolders()).find((f) => f.title === name) ?? null;
  /** The contact with exactly this name; throws when there is none. */
  const contact = (first: string, last: string, title = 'Personal') =>
    one(ws(title).contacts().find((c) => c.firstName === first && c.lastName === last), `contact ${first} ${last}`);
  /** The contact with exactly this name, or `null`. */
  const findContact = (first: string, last: string, title = 'Personal') =>
    each(title).flatMap((w) => w.contacts()).find((c) => c.firstName === first && c.lastName === last) ?? null;
  const tagged = (task: Task, name: string) => task.tags().some((tag) => tag.name.toLowerCase() === name.toLowerCase());
  /** Whether `text` holds every needle, ignoring case. */
  const mentions = (text: string, ...needles: (string | number)[]) =>
    needles.every((needle) => String(text).toLowerCase().includes(String(needle).toLowerCase()));

  const WORDS = ['zero', 'one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine', 'ten',
    'eleven', 'twelve', 'thirteen', 'fourteen', 'fifteen', 'sixteen', 'seventeen', 'eighteen', 'nineteen', 'twenty'];
  const plain = (text: string) => String(text).replace(/(\d),(\d{3})/g, '$1$2');
  /** Whether `text` states the number `n`, in digits (1,500 or 1500) or, up to 20, in words. */
  const saysNumber = (text: string, n: number) =>
    new RegExp(`(^|[^\\d.])${n}(?![\\d])`).test(plain(text)) ||
    (WORDS[n] !== undefined && new RegExp(`\\b${WORDS[n]}\\b`, 'i').test(text));
  /** The numbers `text` states in digits. */
  const numbers = (text: string) => (plain(text).match(/\d+(\.\d+)?/g) ?? []).map(Number);
  const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September',
    'October', 'November', 'December'];
  /** Whether `text` names `when` as YYYY-MM-DD or as "14 March" / "March 14". */
  const saysDate = (text: string, when: Date) => {
    const month = MONTHS[when.getMonth()];
    const d = when.getDate();
    return text.includes(iso(when)) ||
      new RegExp(`\\b${d}(st|nd|rd|th)?\\s+(of\\s+)?${month}|${month}\\s+${d}(st|nd|rd|th)?\\b`, 'i').test(text);
  };
  /** Whether `text` names the local date `offset` days from today. */
  const saysDay = (text: string, offset: number) => saysDate(text, date(offset));
  /** Whether the reply says it found nothing, rather than making something up. */
  const saysNone = (text: string) =>
    /\b(no|not|couldn'?t|can'?t|cannot|unable|don'?t|doesn'?t|isn'?t|nothing|none|haven'?t|aren'?t)\b/i.test(
      text.replace(/[\u2018\u2019]/g, "'"),
    );

  const SNAPSHOT_KEY = 'bench_snapshot';
  type Row = (string | number | null | string[])[];
  interface WorkspaceSnapshot {
    title: string;
    tasks: Row[];
    documents: Row[];
    taskFolders: Row[];
    documentFolders: Row[];
    contacts: unknown[][];
    tags: string[];
  }
  const snapshotOf = (): WorkspaceSnapshot[] =>
    cohesion.workspaces().map((w) => ({
      title: w.title,
      tasks: w.tasks({ status: ALL }).map((t) => [
        t.id, t.title, t.status, t.priority, t.size, t.dueDate, t.startDate, t.taskFolderId,
        t.description, JSON.stringify(t.assignee), t.tags().map((tag) => tag.name).sort(),
      ]),
      documents: w.documents().map((d) => [d.id, d.title, d.contents, d.folderId]),
      taskFolders: w.taskFolders().map((f) => [f.id, f.title]),
      documentFolders: w.documentFolders().map((f) => [f.id, f.title, f.parentId]),
      contacts: w.contacts().map((c) => [
        c.id, c.firstName, c.middleName, c.lastName, c.notes, c.methods.map((m) => [m.kind, m.value]),
      ]),
      tags: w.tags().map((tag) => tag.name),
    }));
  /** Records the starting state; the bench calls it after every setup. */
  const remember = () => ws().setMetadata(SNAPSHOT_KEY, JSON.stringify(snapshotOf()));
  /** Whether nothing the agent could change has changed since the setup. */
  const unchanged = () => ws().metadata(SNAPSHOT_KEY) === JSON.stringify(snapshotOf());
  /** A workspace as the setup left it. */
  const before = (title = 'Personal'): WorkspaceSnapshot => {
    const all = JSON.parse(ws().metadata(SNAPSHOT_KEY) ?? '[]') as WorkspaceSnapshot[];
    const found = all.find((w) => w.title === title);
    if (!found) throw new Error(`no workspace "${title}" at the start`);
    return found;
  };
  /** The ids of what the setup left of `kind`, in the workspace or, for `ANY`, every one. */
  const oldIds = (kind: 'tasks' | 'documents', title: string) =>
    new Set(each(title).flatMap((w) => before(w.title)[kind].map(([id]) => id)));
  /** The workspace's tasks made since the setup, in any status. */
  const newTasks = (title = 'Personal') => {
    const old = oldIds('tasks', title);
    return tasks(title).filter((t) => !old.has(t.id));
  };
  /** The workspace's documents made since the setup. */
  const newDocs = (title = 'Personal') => {
    const old = oldIds('documents', title);
    return each(title).flatMap((w) => w.documents()).filter((d) => !old.has(d.id));
  };
  /** The titles of the workspace's tasks that differ from the setup's. */
  const changedTasks = (title = 'Personal') => {
    const old = new Map(before(title).tasks.map((row) => [row[0], JSON.stringify(row)]));
    const now = snapshotOf().find((w) => w.title === title)?.tasks ?? [];
    return now
      .filter((row) => old.has(row[0]) && old.get(row[0]) !== JSON.stringify(row))
      .map((row) => String(row[1]));
  };
  /** Whether the tasks changed since the setup are exactly `titles`. */
  const changedOnly = (titles: string[], title = 'Personal') =>
    JSON.stringify(changedTasks(title).sort()) === JSON.stringify([...titles].sort());
  /** `[changedOnly(titles), what]`, ready for `check`. */
  const onlyChanged = (...titles: string[]): Check => [
    changedOnly(titles),
    `changed only ${titles.join(', ')} (changed: ${changedTasks().join(', ') || 'nothing'})`,
  ];

  /** `[ok, what]`: whether something holds, and what it is. */
  type Check = readonly unknown[];
  /**
   * A verdict from `[ok, what]` pairs: nothing when every `ok` holds,
   * which passes, or a string naming each `what` that didn't, which fails.
   */
  const check = (checks: readonly Check[]): string | undefined => {
    const missed = checks.filter(([ok]) => !ok).map(([, what]) => String(what));
    return missed.length ? `missed: ${missed.join('; ')}` : undefined;
  };
  /** A read-only question's verdict: the answer's checks, and nothing changed. */
  const answered = (checks: readonly Check[]) => check([...checks, [unchanged(), 'changed nothing']]);

  /** Applies a starting state; the bench calls it first in every setup. */
  const load = (state: StartingState) => {
    for (const w of state.workspaces) {
      const workspace = ws(w.title);
      for (const name of w.tags ?? []) workspace.createTag(name);
      const folders = Object.fromEntries(
        (w.taskFolders ?? []).map((title) => [title, workspace.createTaskFolder({ title })]),
      );
      const contacts = (w.contacts ?? []).map((c) =>
        workspace.createContact({ firstName: c.firstName, lastName: c.lastName, methods: c.methods, notes: c.notes }),
      );
      for (const t of w.tasks ?? []) {
        const options: TaskCreateOptions = { title: t.title };
        if (t.folder) options.folder = folders[t.folder];
        if (t.priority) options.priority = t.priority;
        if (t.size) options.size = t.size;
        if (t.start !== undefined && t.start !== null) options.startDate = day(t.start);
        if (t.due !== undefined && t.due !== null) options.dueDate = day(t.due);
        if (t.description) options.description = t.description;
        const assignee = t.assignee;
        if (assignee?.kind === 'user') options.assignee = { kind: 'user' };
        if (assignee?.kind === 'contact') {
          const who = contacts.find((c) => c.firstName === assignee.firstName && c.lastName === assignee.lastName);
          if (!who) throw new Error(`no contact ${assignee.firstName} ${assignee.lastName} to assign "${t.title}" to`);
          options.assignee = { kind: 'contact', contactId: who.id };
        }
        const task = workspace.createTask(options);
        for (const tag of t.tags ?? []) task.tag(tag);
        if (t.status === 'completed') task.complete();
        if (t.status === 'cancelled') task.cancel();
      }
      const fill = (parent: DocumentFolder | null, f: DocumentFolderState) => {
        const folder = parent ? parent.createFolder(f.title) : workspace.createDocumentFolder(f.title);
        for (const d of f.documents ?? []) folder.createDocument(d.title, d.contents);
        for (const inner of f.folders ?? []) fill(folder, inner);
      };
      for (const f of w.documentFolders ?? []) fill(null, f);
      for (const d of w.documents ?? []) workspace.createDocument(d.title, d.contents);
      for (const j of w.journals ?? []) workspace.journal(day(j.day)).contents = j.contents;
    }
  };

  return {
    ALL, ANY, iso, date, day, ws, tasks, task, tasksLike, doc, docsLike, taskFolder, findTaskFolder, docFolder,
    findDocFolder, contact, findContact, tagged,
    mentions, saysNumber, numbers, saysDate, saysDay, saysNone, remember, unchanged, before, newTasks, newDocs,
    changedTasks, changedOnly, onlyChanged, check, answered, load,
  };
})();
