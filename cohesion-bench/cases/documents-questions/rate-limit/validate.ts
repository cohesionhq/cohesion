export default function validate() {
  return B.answered([
    [B.saysNumber(reply, 600), '600 a minute'],
  ])
}
