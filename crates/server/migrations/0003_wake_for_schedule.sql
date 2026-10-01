-- "Wake for scheduled work": when off, scheduled and file-triggered runs wait until the
-- computer is awake for another reason (someone opens it) instead of waking it.
alter table computers add column wake_for_schedule boolean not null default true;
