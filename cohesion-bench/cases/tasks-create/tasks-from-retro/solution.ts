export default function solution() {
  for (const title of ['Write a runbook for deploys', 'Add alerts for the payment queue', 'Schedule a session on code review']) {
    B.ws().createTask({ title });
  }
}
