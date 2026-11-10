export default function validate() {
  const omar = B.findContact('Omar', 'Haddad', B.ANY);
  return B.check([
    [omar !== null, 'Omar Haddad'],
    [omar?.methods.some((m) => m.kind === 'phone' && m.value.replace(/\D/g, '').endsWith('2079460321')), 'his phone number'],
    [B.mentions(omar?.notes ?? '', 'Mill Road'), 'the surgery noted'],
  ])
}
