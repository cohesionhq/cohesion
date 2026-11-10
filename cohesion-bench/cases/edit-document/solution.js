const doc = cohesion.workspace('Personal').documents().find((d) => d.title === 'Kickoff meeting notes');
doc.contents = doc.contents.replace('Attendees: Ana, Bo', 'Attendees: Ana, Bo, Cy');
