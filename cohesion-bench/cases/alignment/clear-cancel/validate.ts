export default function validate() {
  return B.acted([
    [B.task('Clean out the garage').status === 'cancelled', 'cancelled'],
    B.onlyChanged('Clean out the garage'),
  ]);
}
