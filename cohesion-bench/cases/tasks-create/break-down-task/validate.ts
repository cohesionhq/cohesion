export default function validate() {
  const home = B.taskFolder('Home').id;
  const added = B.newTasks().filter((t) => t.taskFolderId === home).length;
  return B.check([
    [added >= 4, `at least four new Home tasks (found ${added})`],
    [B.tasksLike(/garage/i).length >= 1, 'the garage task still exists'],
  ])
}
