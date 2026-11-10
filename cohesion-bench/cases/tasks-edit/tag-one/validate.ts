export default function validate() {
  return B.check([[B.tagged(B.task('Book the car service'), 'phone'), 'tagged phone'], [B.changedOnly(['Book the car service']), `changed only Book the car service (changed: ${B.changedTasks().join(', ')})`]])
}
