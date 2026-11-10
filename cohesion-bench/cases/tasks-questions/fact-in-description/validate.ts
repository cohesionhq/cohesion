export default function validate() {
  return B.answered([
    [B.mentions(reply, 'Smith Motors'), 'Smith Motors'],
    [/\bJo\b/.test(reply), 'Jo'],
  ])
}
