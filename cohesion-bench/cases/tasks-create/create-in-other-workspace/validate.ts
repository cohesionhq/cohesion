export default function validate() {
  const acme = B.tasksLike(/OKR/i, 'Acme');
  return B.check([
    [acme.length === 1 && acme[0].taskFolderId === B.taskFolder('Backlog', 'Acme').id, "in Acme's Backlog"],
    [B.tasksLike(/OKR/i).length === 0, 'nothing added to Personal'],
  ])
}
