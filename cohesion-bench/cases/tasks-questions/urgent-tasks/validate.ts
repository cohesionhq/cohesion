export default function validate() {
  return B.answered([
    [B.mentions(reply, 'tax'), 'the tax return'],
  ])
}
