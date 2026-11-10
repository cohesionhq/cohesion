export default function validate() {
  const open = B.tasksLike(/milk/i).filter((t) => t.status === 'incomplete');
  return B.check([
    [open.length === 1, `one open milk task (found ${open.length})`],
    [/already|exist/i.test(reply), 'told the user it is already there'],
  ])
}
