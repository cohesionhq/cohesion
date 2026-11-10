export default function validate() {
  const danas = B.ws().contacts().filter((c) => c.firstName === 'Dana' && c.lastName === 'Wu');
  const values = danas[0]?.methods.map((m) => m.value) ?? [];
  return B.check([
    [danas.length === 1, `one Dana Wu (found ${danas.length})`],
    [values.includes('dana@example.com'), 'the email'],
    [values.includes('+44 7700 900555'), 'the phone number'],
  ])
}
