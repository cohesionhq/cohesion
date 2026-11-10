export default function validate() {
  const created = bench.cues.filter((cue) => cue.hook === 'task:create');
  const subject = (priority: string) => ({ title: 'Test', priority, status: 'incomplete', workspaceId: 1, id: 1 });
  const fires = (cue: CueLeft, priority: string) => {
    try { return Boolean(eval(`(${cue.code})`)(subject(priority))); } catch { return false; }
  };
  return B.check([
    [created.length >= 1, `a task:create cue (cues: ${bench.cues.map((c) => c.hook).join(', ') || 'none'})`],
    [created.some((cue) => fires(cue, 'urgent') && !fires(cue, 'low') && !fires(cue, 'none')), 'fires for an urgent task only'],
  ])
}
