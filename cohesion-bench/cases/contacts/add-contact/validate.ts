export default function validate() {
  const ada = B.findContact('Ada', 'Lovelace', B.ANY);
  return B.check([[ada?.methods.some((m) => m.kind === 'email' && m.value === 'ada@example.com'), 'Ada Lovelace with her email']])
}
