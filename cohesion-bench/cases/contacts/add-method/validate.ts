export default function validate() {
  const values = B.contact('Priya', 'Shah').methods.map((m) => m.value);
  return B.check([
    [values.includes('priya.shah@work.example'), 'the work email'],
    [values.includes('priya@example.com') && values.includes('+44 7700 900111'), 'the old email and phone kept'],
  ])
}
