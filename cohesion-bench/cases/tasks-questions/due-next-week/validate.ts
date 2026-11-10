export default function validate() {
  return B.answered([
    [B.mentions(reply, 'tap'), 'the kitchen tap'],
    [B.mentions(reply, 'dentist'), 'the dentist'],
    [!B.mentions(reply, 'Piranesi'), 'not Piranesi, due later'],
  ])
}
