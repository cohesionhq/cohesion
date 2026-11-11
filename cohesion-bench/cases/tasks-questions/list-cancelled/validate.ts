export default function validate() {
  return B.answered([
    [B.mentions(reply, 'couch'), 'the Couch to 5k plan'],
  ])
}
