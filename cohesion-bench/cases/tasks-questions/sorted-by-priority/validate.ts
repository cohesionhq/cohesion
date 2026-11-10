export default function validate() {
  return B.answered([
    [B.mentions(reply, 'dentist') && B.mentions(reply, 'gym'), 'the dentist and the gym'],
    [reply.toLowerCase().indexOf('dentist') < reply.toLowerCase().indexOf('gym'), 'the dentist (high) before the gym (medium)'],
    [reply.toLowerCase().indexOf('gym') < reply.toLowerCase().indexOf('lens'), 'the gym before the contact lenses (none)'],
  ])
}
