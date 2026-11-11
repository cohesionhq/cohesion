export default function validate() {
  const [task] = B.tasksLike(/passport/i);
  return B.check([
    [B.tasksLike(/passport/i).length === 1, 'one passport task'],
    [task?.taskFolderId === B.taskFolder('Errands').id, 'in Errands'],
  ])
}
