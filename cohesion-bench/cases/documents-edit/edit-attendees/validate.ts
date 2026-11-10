export default function validate() {
  const text = B.doc('Kickoff meeting notes').contents;
  const attendees = text.split('\n').find((line) => /attendees/i.test(line)) ?? '';
  return B.check([
    [/Ana/.test(attendees) && /\bBo\b/.test(attendees) && /\bCy\b/.test(attendees), `attendees: "${attendees}"`],
    [B.mentions(text, 'Launch the beta on 1 December', 'Weekly check-ins on Tuesdays', 'Ana owns the launch checklist'), 'the decisions kept'],
  ])
}
