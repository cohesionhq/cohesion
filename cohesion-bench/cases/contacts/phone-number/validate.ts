export default function validate() {
  return B.answered([
    [reply.replace(/\D/g, '').includes('447700900111') || reply.replace(/\D/g, '').includes('07700900111'), 'her number'],
  ])
}
