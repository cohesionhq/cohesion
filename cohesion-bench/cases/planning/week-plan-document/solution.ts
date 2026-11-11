export default function solution() {
  const lines = ['# This week', ''];
  for (let n = 0; n < 7; n++) {
    lines.push(`## ${B.date(n).toLocaleDateString('en-GB', { weekday: 'long' })} ${B.day(n)}`, '');
    for (const t of B.ws().tasks({ status: 'incomplete' }).filter((t) => t.dueDate === B.day(n))) lines.push(`- ${t.title}`);
    lines.push('');
  }
  B.ws().createDocument('This week', lines.join('\n'));
}
