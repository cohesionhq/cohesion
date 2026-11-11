export default function validate() {
  return B.answered([
    [B.mentions(reply, 'kneeler') || B.mentions(reply, 'Cleeves'), 'the kneeler or the Ann Cleeves novel'],
  ])
}
