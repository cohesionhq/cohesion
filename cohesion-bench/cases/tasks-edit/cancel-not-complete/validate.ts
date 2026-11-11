export default function validate() {
  return B.check([
    [B.task('Sign up for the Spanish evening course').status === 'cancelled', `cancelled, not completed (was ${B.task('Sign up for the Spanish evening course').status})`],
    [B.changedOnly(['Sign up for the Spanish evening course']), `changed only Sign up for the Spanish evening course (changed: ${B.changedTasks().join(', ')})`],
  ])
}
