export default function validate() {
  return B.askedThen([
    [B.task('Call mum').status === 'completed', 'Call mum done'],
    B.onlyChanged('Call mum'),
  ]);
}
