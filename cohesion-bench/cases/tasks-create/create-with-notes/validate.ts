export default function validate() {
  const [task] = B.tasksLike(/porto/i);
  const notes = task?.description ?? '';
  return B.check([
    [B.tasksLike(/porto/i).length === 1, 'one Porto task'],
    [B.mentions(notes, '12') && B.mentions(notes, '19') && /mar/i.test(notes), `the dates in its notes (notes: "${notes}")`],
  ])
}
