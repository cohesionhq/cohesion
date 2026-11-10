export default function validate() {
  return B.answered([
    [B.mentions(reply, 'shed'), 'behind the shed'],
  ])
}
