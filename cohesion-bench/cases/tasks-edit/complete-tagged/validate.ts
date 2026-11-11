export default function validate() {
  const errands = ['Buy printer ink', 'Pick up dry cleaning', 'Return library books', 'Get a spare house key cut'];
  const changed = B.changedTasks();
  return B.check([
    ...errands.map((title) => [B.task(title).status === 'completed', `${title} done`]),
    [changed.every((title) => errands.includes(title) || title === 'Buy milk'), `nothing else changed (changed: ${changed.join(', ')})`],
  ])
}
