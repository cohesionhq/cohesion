export default function validate() {
  return B.answered([
    [/KX21\s*BNM/i.test(reply), 'KX21 BNM'],
  ])
}
