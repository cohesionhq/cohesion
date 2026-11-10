export default function validate() {
  return B.answered([
    [reply.replace(/\D/g, '').includes('4155550142'), '+1 415 555 0142'],
  ])
}
