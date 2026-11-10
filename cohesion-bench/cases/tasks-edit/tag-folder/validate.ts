export default function validate() {
  const open = ['File the tax return', 'Cancel the old phone contract', 'Check the electricity bill', 'Claim back the train fare'];
  return B.check([
    ...open.map((title) => [B.tagged(B.task(title), 'money'), `${title} tagged money`]),
    [B.tasks().filter((t) => B.tagged(t, 'money') && !open.includes(t.title) && t.title !== 'Set up the ISA transfer').length === 0, 'no other task tagged'],
  ])
}
