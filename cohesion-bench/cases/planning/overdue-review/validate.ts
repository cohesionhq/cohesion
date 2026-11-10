export default function validate() {
  const [doc] = B.docsLike(/overdue review/i, B.ANY);
  const text = doc?.contents ?? '';
  return B.check([
    [B.task('Cancel the old phone contract').dueDate === B.day(1), 'the high-priority phone contract moved to tomorrow'],
    B.onlyChanged('Cancel the old phone contract'),
    [B.mentions(text, 'library books') && B.mentions(text, 'gym') && B.mentions(text, 'phone contract'), 'all three in the review'],
  ]);
}
