export default function validate() {
  const list = B.doc('Reading list').contents;
  const items = list.split('\n').filter((line) => /^\s*[-*+]\s/.test(line));
  const one = (pattern: RegExp) => B.tasksLike(pattern, B.ANY).filter((t) => t.status === 'incomplete').length === 1;
  return B.check([
    [one(/overstory/i), 'one task for The Overstory'],
    [one(/klara/i), 'one task for Klara and the Sun'],
    [one(/piranesi/i), 'still one Piranesi task'],
    [items.some((line) => /babel/i.test(line)), 'Babel on the list'],
    [B.mentions(list, 'Piranesi', 'The Overstory', 'Klara and the Sun'), 'the list kept'],
  ]);
}
