-- Schedules run in the computer's time zone ("every day at 9 AM" means 9 AM there).
-- A person picks theirs at sign-up; a new computer takes its owner's; each job carries
-- its computer's so the scheduler needs no join.
alter table users add column time_zone text not null default 'America/New_York';
alter table computers add column time_zone text not null default 'America/New_York';
alter table jobs add column time_zone text not null default 'America/New_York';
