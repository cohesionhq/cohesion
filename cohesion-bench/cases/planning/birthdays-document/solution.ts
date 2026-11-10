export default function solution() {
  const next = (month: number, dayOfMonth: number) => {
    const at = new Date(B.date(0).getFullYear(), month, dayOfMonth, 12);
    if (at < B.date(0)) at.setFullYear(at.getFullYear() + 1);
    return at.getTime();
  };
  const lines = [
    { at: next(2, 14), line: '- Priya Shah: 14 March' },
    { at: next(7, 2), line: '- Sam Lee: 2 August' },
    { at: next(10, 30), line: '- Helen Carter (Mum): 30 November' },
  ].sort((a, b) => a.at - b.at).map((l) => l.line);
  B.ws().createDocument('Birthdays', `# Birthdays\n\n${lines.join('\n')}\n`);
}
