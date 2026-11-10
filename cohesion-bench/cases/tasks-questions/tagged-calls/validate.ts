export default function validate() {
  return B.answered([
    [B.mentions(reply, 'Bob'), 'Bob'],
    [B.mentions(reply, 'dentist'), 'the dentist'],
    [B.mentions(reply, 'phone contract'), 'the phone contract'],
    [/\bmum\b/i.test(reply), 'Mum'],
  ])
}
