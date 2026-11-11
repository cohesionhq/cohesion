export default function validate() {
  const notes = B.contact('Sam', 'Lee').notes;
  return B.check([
    [B.mentions(notes, 'Oakfield'), 'the school'],
    [B.mentions(notes, 'Mia', 'number 12', '2 August'), 'the earlier notes kept'],
  ])
}
