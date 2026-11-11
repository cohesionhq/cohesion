export default function solution() {
  const lines = (tasks: Task[]) => tasks.map((t) => `- ${t.title} (${t.dueDate})`).join('\n');
  B.ws().createDocument('Weekly review', '## Overdue\n\n' + lines(B.ws().tasks({ dueDate: { before: B.day(0) } })) +
    '\n\n## Due this week\n\n' + lines(B.ws().tasks({ dueDate: { after: B.day(-1), before: B.day(8) } })));
}
