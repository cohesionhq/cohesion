const doc = cohesion.workspace('Personal').documents().find((d) => d.title === 'Kickoff meeting notes');
const text = doc ? doc.contents : '';
const attendees = text.split('\n').find((line) => /attendees/i.test(line)) ?? '';
const kept = text.includes('Ship the beta on 1 December') && text.includes('Weekly check-ins on Tuesdays');
({
  pass: /Ana/.test(attendees) && /Bo\b/.test(attendees) && /Cy\b/.test(attendees) && kept,
  reason: `attendees line: "${attendees}"${kept ? '' : '; the decisions were lost'}`,
})
