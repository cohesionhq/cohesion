const w = cohesion.workspace('Personal');
for (const title of [
  'Draft the quarterly report',
  'Collect Q3 numbers for the quarterly report',
  'Send the quarterly report to Dana',
  'Book the dentist',
  'Renew the car insurance',
  'Plan the team offsite',
]) {
  w.createTask({ title });
}
