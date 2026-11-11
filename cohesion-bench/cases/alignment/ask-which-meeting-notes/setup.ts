export default function setup() {
  const meetings = B.docFolder('Meetings');
  meetings.createDocument('Budget meeting notes', '# Budget meeting\n\nAttendees: Dev, Priya\n\n## Decisions\n\n- Cap the trip spend at €1,500\n');
  meetings.createDocument('Street party meeting notes', '# Street party meeting\n\nAttendees: Sam, Rose\n\n## Decisions\n\n- Hold it on the first Saturday of June\n');
}
