export default function validate() {
  return B.answered([
    [B.saysDay(reply, -3) || /three days ago/i.test(reply), 'three days ago'],
  ])
}
