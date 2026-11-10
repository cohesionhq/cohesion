export default function validate() {
  return B.acted([
    [B.task('Get a flu jab').priority !== 'none', `a priority set (is ${B.task('Get a flu jab').priority})`],
    B.onlyChanged('Get a flu jab'),
  ]);
}
