export default function validate() {
  const [doc] = B.docsLike(/weekly review/i, B.ANY);
  const text = doc?.contents ?? '';
  return B.check([
    [B.mentions(text, 'library books', 'gym', 'phone contract'), 'the three overdue tasks'],
    [B.mentions(text, 'tap', 'dentist'), 'the tap and the dentist, due this week'],
    [!B.mentions(text, 'tax return'), 'not the tax return, due later'],
  ])
}
