export default function validate() {
  return B.answered([
    [B.saysNumber(reply, 600), '600'],
    [/sign(s|ed)?\s+(the user\s+)?out/i.test(reply), 'signs the user out'],
  ])
}
