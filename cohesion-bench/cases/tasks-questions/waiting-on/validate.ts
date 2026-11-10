export default function validate() {
  return B.answered([
    [B.mentions(reply, 'landlord'), 'the landlord'],
  ])
}
