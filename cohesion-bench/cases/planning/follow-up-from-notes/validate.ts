export default function validate() {
  const rotate = B.tasksLike(/credential/i);
  const drives = B.tasksLike(/drive/i);
  return B.check([
    [rotate.length === 1 && rotate[0].dueDate === '2026-12-05', 'the credentials, due 2026-12-05'],
    [drives.length === 1 && drives[0].dueDate === '2026-12-08', 'the drives, due 2026-12-08'],
    [B.newTasks().length === 2, `only those two (made ${B.newTasks().length})`],
  ])
}
