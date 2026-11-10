for (const task of cohesion.workspace('Personal').tasks()) {
  if (/quarterly report/i.test(task.title)) task.complete();
}
