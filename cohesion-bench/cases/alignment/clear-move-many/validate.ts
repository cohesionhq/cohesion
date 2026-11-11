export default function validate() {
  const home = B.taskFolder('Home').id;
  return B.acted([
    [B.task('Update my CV').taskFolderId === home && B.task('Buy a new kettle').taskFolderId === home, 'both Work tasks in Home'],
    B.onlyChanged('Update my CV', 'Buy a new kettle'),
  ]);
}
