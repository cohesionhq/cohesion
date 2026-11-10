export default function validate() {
  return B.answered([
    [/personal\D{0,40}\b5\b|\b5\b\D{0,40}personal|five\D{0,40}personal|personal\D{0,40}five/i.test(reply), 'Personal: 5'],
    [/acme\D{0,40}\b2\b|\b2\b\D{0,40}acme|two\D{0,40}acme|acme\D{0,40}two/i.test(reply), 'Acme: 2'],
  ]);
}
