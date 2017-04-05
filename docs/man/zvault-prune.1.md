zvault-prune(1) -- Remove backups based on age
==============================================

## SYNOPSIS

`zvault prune [OPTIONS] [REPO]`


## DESCRIPTION

This subcommand removes backups in the repository `REPO` based on their age.

If `REPO` is omitted, the default repository location is used instead.

If a prefix is specified via `--prefix`, only backups which start with this
string are considered for removal.

The prune logic will preserve a certain number of backups for different time
periods and discard the rest. The available periods are `daily`, `weekly`,
`monthly` and `yearly`. For each of those periods, a number `N` can be specified
that defines that for each of the last `N` of these periods, a single backup
(the newest one in that period) will be kept.

For example, `--daily 3` will keep backups of the last 3 days, i.e. one backup
for today, yesterday and the day before yesterday (if a backup has been saved
today). If several backups have been saved on a single day, only the newest is
kept.

The different periods can also be combined to preserve backups using multiple
different time periods. Backups are only removed if they are not preserved by
any of the time periods.

For example, `--daily 3 --weekly 4 --monthly 3` will keep one backup for each of
the last 3 days, for each of the last 4 weeks and for each of the last 3 months.
As time progresses, the daily backups will be removed as new ones are created so
that only 3 of them are kept but each week one of them will be preserved as a
weekly backup and an old weekly backup will be removed unless that backup
happens to be the last backup of last month...

If one period is not set, no backups for that time period will be preserved.
This command will refuse to remove all backups if called without options.

Unless the option `--force` is set, this command only displays the backups that
would be removed but does not remove them.

This command renders certain chunks unused, but reclaiming their space is a
complicated task as chunks are combined into bundles together with other chunks
which are potentially still used. Please use _zvault-vacuum(1)_ to reclaim
unused space.

**Important note: Although this command does not actually remove any data, the
data of the deleted backups becomes inaccessible and can not be restored.**


## OPTIONS

  * `-p`, `--prefix <PREFIX>`:

    Only consider backups starting with this prefix.


  * `-d`, `--daily <NUM>`:

    Keep the newest backup for each of the last `NUM` days.


  * `-w`, `--weekly <NUM>`:

    Keep the newest backup for each of the last `NUM` weeks.


  * `-m`, `--monthly <NUM>`:

    Keep the newest backup for each of the last `NUM` months.


  * `-y`, `--yearly <NUM>`:

    Keep the newest backup for each of the last `NUM` years.


  * `-f`, `--force`:

    Actually remove backups instead of displaying what would be removed.


  * `-h`, `--help`:

    Prints help information


## COPYRIGHT

Copyright (C) 2017  Dennis Schwerdel
This software is licensed under GPL-3 or newer (see LICENSE.md)
