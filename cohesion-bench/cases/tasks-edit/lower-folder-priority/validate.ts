export default function validate() {
  return B.check([
    [B.task('Update my CV').priority === 'low', 'CV low'],
    [B.task('Buy a new kettle').priority === 'low', 'kettle low'],
    [B.changedOnly(['Update my CV', 'Buy a new kettle']), `changed only Update my CV, Buy a new kettle (changed: ${B.changedTasks().join(', ')})`],
  ])
}
