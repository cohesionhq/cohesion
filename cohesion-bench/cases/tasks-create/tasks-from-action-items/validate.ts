export default function validate() {
  const rotate = B.tasksLike(/rotat|credential/i, B.ANY);
  const drives = B.tasksLike(/drive/i, B.ANY);
  return B.check([
    [rotate.length === 1 && rotate[0].dueDate === '2026-12-05', 'rotate the credentials, due 2026-12-05'],
    [drives.length === 1 && drives[0].dueDate === '2026-12-08', 'order the drives, due 2026-12-08'],
    [B.tasksLike(/incident/i, B.ANY).length === 0, "no task for Bo's incident summary"],
  ])
}
