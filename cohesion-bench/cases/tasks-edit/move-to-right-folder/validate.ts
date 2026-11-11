export default function validate() {
  return B.check([
    [B.task('Buy a new kettle').taskFolderId === B.taskFolder('Home').id, 'in Home'],
    [B.changedOnly(['Buy a new kettle']), `changed only Buy a new kettle (changed: ${B.changedTasks().join(', ')})`],
  ])
}
