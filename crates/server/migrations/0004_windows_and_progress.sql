-- A schedule can run only within part of the day ("every 3 hours, from 6 AM to midnight";
-- the Watcher's "only during these hours"), in minutes of the job's local day. An end before
-- the start wraps past midnight; 1440 is midnight.
alter table jobs add column window_start int;
alter table jobs add column window_end int;
-- "Tell me when it finishes", ticked on one run.
alter table runs add column tell_me boolean;
-- "Wait a minute for more files": a file-triggered run waits until changes settle.
alter table jobs add column settle_secs int not null default 0;
