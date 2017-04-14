zvault-backup(1) -- Create a new backup
=======================================

## SYNOPSIS

`zvault backup [OPTIONS] <SRC> <BACKUP>`


## DESCRIPTION

This subcommand creates a new backup `BACKUP` from the data located at `SRC`.

The backup given by `BACKUP` must be in the format `[repository]::backup_name`
as described in _zvault(1)_. If `repository` is omitted, the default repository
location is used instead.

The source data given by `SRC` can either be a filesystem path or the path of a
tar archive (with `--tar`).

If `SRC` is a filesystem path, a reference backup is used (unless `--full` is
set) to compare the data with and only store modified data and take the
unmodified data from the reference backup. Unless a specific reference backup
is chosen via `--ref`, the latest matching backup from the same machine with the
same source path is used as reference.

When `SRC` is a filesystem path, a set of exclude patterns can be configured.
The patterns can be given directly via `--exclude` or be read from a file via
`--excludes-from`. Unless `--no-default-excludes` is set, a set of default
exclude pattern is read from the file `excludes` in the repository folder.
All exclude pattern given via any of these ways will be combined.

If `--tar` is specified and `SRC` is `-`, the input is read from stdin.

Unless `--xdev` is set, zVault will not traverse into subfolders that are on a
different filesystem, i.e. mount points will not be included.

When zVault fails to read a source file, either because of file permissions,
filesystem errors or because the file has an unsupported type, it will print a
warning message and continue with the backup process.

zVault will store all file attributes including extended attributes except for
creation time and access time as creation time can not be reliably set on
restore and access times change by reading files.


## OPTIONS

  * `-e`, `--exclude <PATTERN>...`:

    Exclude this path or file pattern. This option can be given multiple times.
    Please see *EXCLUDE PATTERNS* for details on pattern.

    This option conflicts with `--tar`.


  * `--excludes-from <FILE>`:

    Read the list of excludes from this file.
    Please see *EXCLUDE PATTERNS* for details on pattern.

    This option conflicts with `--tar`.


  * `--full`:

    Create a full backup without using another backup as a reference. This makes
    sure that all files in the source path (except excluded files) are fully
    read. The file contents will still be deduplicated by using existing backups
    but all files are read fully.

    This option conflicts with `--ref`.


  * `--no-default-excludes`:

    Do not load the default `excludes` file from the repository folder.
    Those excludes are pre-filled with generic pattern to exclude like pseudo
    filesystems or cache folders.


  * `--ref <REF>`:

    Base the new backup on this reference backup instead of automatically
    selecting a matching one. The backup given as `REF` must be a valid backup
    name as listed by zvault-list(1).

    This option conflicts with `--full`.


  * `--tar`:

    Read the source data from a tar archive instead of the filesystem. When this
    flag is set, the `SRC` path must specify a valid tar file.
    The contents of the archive are then read instead of the filesystem. Note
    that the tar file contents are read as files and directories and not just
    as a single file (this would happen when `SRC` is a tar file and `--tar` is
    not set).

    This option can be used to import a backup that has been exported using
    zvault-restore(1) with the `--tar` flag.

    This flag conflicts with `--exclude` and `--excludes_from`.


  * `-x`, `--xdev`:

    Allow to cross filesystem boundaries. By default, paths on different
    filesystems than the start path will be ignored. If this flag is set,
    the scan will traverse also into mounted filesystems.
    **Note:** Please use this option with case. Some pseudo filesystems
    contain arbitrarily deep nested directories that will send zVault into
    an infinite loop. Also it should be avoided to include the remote storage
    in the backup.


  * `-h`, `--help`:

    Prints help information


## EXCLUDE PATTERNS

Exclude patterns can either be absolute patterns or relative patterns. Absolute
patterns start with `/` and must match from the begin of the absolute file path.
Relative patterns start with anything but `/` and can also match any portion of
the absolute path. For example the pattern `/bin` only matches the system
directory `/bin` but not `/usr/bin` or `/usr/local/bin` while the pattern `bin`
matches them too.

Exclude patterns must match full path components, i.e. the pattern `bin` will
match any path that contains `bin` as as component (e.g. `/bin` and `/usr/bin`)
but not paths that contain `bin` only as substring like `/sbin`.

Wildcards can be used to match also substrings of path components:

- `?` matches any single character.
- `*` matches any string not containing `/`, i.e. `*` only matches within a path
  component but does not span components. For example `/usr/*bin` matches
  `/usr/bin` and `/usr/sbin` but not `/usr/local/bin`.
- `**` matches any string, even spanning across path components. So `/usr/**bin`
  will match `/usr/bin`, `/usr/sbin` and also `/usr/local/bin`.

If a pattern matches on a filesystem entry, that entry and any child entry (in
the case of directories) will be left out of the backup.


## COPYRIGHT

Copyright (C) 2017  Dennis Schwerdel
This software is licensed under GPL-3 or newer (see LICENSE.md)
