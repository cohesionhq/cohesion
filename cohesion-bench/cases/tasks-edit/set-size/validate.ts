export default function validate() {
  return B.check([[B.task('Update my CV').size === 1, 'size 1'], [B.changedOnly(['Update my CV']), `changed only Update my CV (changed: ${B.changedTasks().join(', ')})`]])
}
