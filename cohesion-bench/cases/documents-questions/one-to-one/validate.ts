export default function validate() {
  return B.answered([
    [/\bTom\b/.test(reply), 'Tom'],
  ])
}
