export default function validate() {
  return B.answered([
    [B.mentions(reply, 'Priya'), 'Priya'],
  ])
}
