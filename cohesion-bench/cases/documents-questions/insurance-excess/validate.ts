export default function validate() {
  return B.answered([
    [/£?\s*250/.test(reply), '£250'],
  ])
}
