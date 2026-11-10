export default function validate() {
  return B.answered([
    [B.saysNumber(reply, 3), 'said 3'],
    [B.mentions(reply, 'Postgres'), 'the Postgres upgrade'],
  ])
}
