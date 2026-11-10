export default function validate() {
  return B.answered([
    [/250\s*g/i.test(reply), '250 g'],
  ])
}
