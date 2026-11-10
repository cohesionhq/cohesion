export default function validate() {
  return B.answered([
    [B.mentions(reply, 'conference ticket'), 'the conference ticket'],
  ])
}
