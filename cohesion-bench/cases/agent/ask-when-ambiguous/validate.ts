export default function validate() {
  return B.check([
    [B.unchanged(), 'changed nothing while it was unclear which call'],
    [bench.asked.length > 0 || /\?/.test(reply), 'asked which call'],
  ])
}
