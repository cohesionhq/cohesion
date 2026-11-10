export default function validate() {
  const open = B.tasksLike(/milk/i, B.ANY).filter((t) => t.status === 'incomplete');
  return B.check([
    [open.length === 1, `one open milk task (found ${open.length})`],
    [B.mentions(reply, 'milk'), 'told the user about the milk task'],
  ])
}
