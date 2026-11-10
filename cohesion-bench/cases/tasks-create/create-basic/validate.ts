export default function validate() {
  const found = B.tasksLike(/plumber/i);
  return B.check([
    [found.length === 1, `one task about the plumber (found ${found.length})`],
    [found.length === 1 && found[0].status === 'incomplete', 'still to do'],
  ])
}
