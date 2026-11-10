const w = cohesion.workspace('Personal');
for (const title of ['Write a runbook for deploys', 'Add alerts for the payment queue', 'Schedule a session on code review']) {
  w.createTask({ title });
}
