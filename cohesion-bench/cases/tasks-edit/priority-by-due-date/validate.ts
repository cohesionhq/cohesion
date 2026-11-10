export default function validate() {
  return B.check([
    [B.task('Check the electricity bill').priority === 'high', 'electricity bill high'],
    [B.task('Cancel the old phone contract').priority === 'high', 'phone contract still high'],
    [B.task('File the tax return').priority === 'urgent', 'tax return left urgent'],
    [B.changedOnly(['Check the electricity bill']), `changed only Check the electricity bill (changed: ${B.changedTasks().join(', ')})`],
  ])
}
