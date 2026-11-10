export default function validate() {
  const want = { 'Draft the press release': '2026-12-03', 'Brief the support team': '2026-12-05', 'Send the launch email': '2026-12-12' };
  return B.check(Object.entries(want).map(([title, date]) => [B.task(title).dueDate === date, `${title} due ${date} (was ${B.task(title).dueDate})`]))
}
