export default function validate() {
  const values = B.contact('Sam', 'Lee').methods.map((m) => m.value);
  return B.check([
    [values.includes('sam@newmail.example'), 'the new email'],
    [!values.includes('sam.lee@example.com'), 'the old email gone'],
  ])
}
