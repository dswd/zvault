% Design Document
# Design Document

## Project Goals
The main goal of zVault is to provide a backup solution that is both reliable
and efficient.

Backups should be stored in a way so that they can be restored **reliably**. A
backup that can not be restored is worthless. Backups should be stored in a
**robust** fashion that allows minor changes to remote backup files or losses in
local cache. There should be a way to **verify the integrity** of backups.

Backups should support **remote storage**. Remote backup files should be stored
on a mounted remote storage (e.g. via `rclone mount`). To support this use case,
remote backup files should be handled with only common file operations so that
dumb remote filesystems can be supported.

The backup process should be **fast**, especially in the common case where only
small changes happened since the last backup. This means that zVault should be
able to find an existing backup for reference and use it to detect differences.

The backups should be stored in a **space-efficient and deduplicating** way, to
save storage space, especially in the common case where only small changes
happened since the last backup. The individual backups should be independent of
each other to allow the removal of single backups based on age in a phase-out
scheme.


## Backup process
The main idea of zVault is to split the data into **chunks** which are stored
remotely. The chunks are combined in **bundles** and compressed and encrypted as
a whole to increase the compression ratio and performance.

An **index** stores **hashes** of all chunks together with their bundle id and
position in that bundle, so that chunks are only stored once and can be reused
by later backups. The index is implemented as a memory-mapped file to maximize
the backup performance.

To split the data into chunks a so-called **chunker** is used. The main goal of
the chunker is to create a maximal amount of same chunks when only a few changes
happened in a file. This is especially tricky when bytes are inserted or deleted
so that the rest of the data is shifted. The chunker uses content-dependent
methods to split the data in order to handle those cases.

By splitting data into chunks and storing those chunks remotely as well as in
the index, any stream of data (e.g. file contents) can be represented by a list
of chunk identifiers. This method is used to represent the contents of a file
and store it in the file metadata. This metadata is then encoded as a data
stream and again represented as a chunk list. Directories contain their children
(e.g. files and other directories) by referring to their metadata as a chunk
list. So finally, the whole directory tree of a backup can be represented as the
chunk list of the root directory which is then stored in a separate backup file.


## Saving space
The design of zVault contains multiple ways in which storage space can be saved.

The most important is deduplication which makes sure that chunks are only stored
once. If only few changes happened since the last backup, almost all chunks are
already present in the index and do not have to be written to remote storage.
Depending on how little data has changed since the last backup, this can save up
to 100% of the storage space.

But deduplication also works within the same backup. Depending on data,
deduplication can save about 10%-20% even on new data due to repetitions in the
data.

If multiple systems use the same remote storage, they can benefit from backups
of other machines and use their chunks for deduplication. This is especially
helpful in the case of whole system backups where all systems use the same
operating system.

Finally zVault uses a powerfull compression that achieves about 1/3 space
reduction in common cases to store the bundles.

In total, a whole series of backups is often significantly smaller than the data
contained in any of the individual backups.


## Vacuum process
As backups are removed, some chunks become unused and could be removed to free
storage space. However, as chunks are combined in bundles, they can not be
removed individually and all other backups must also be checked in order to make
sure the chunks are truly unused.

zVault provides an analysis method that scans all backups and identifies unused
chunks in bundles. The vacuum process can then be used to reclaim the space used
by those chunks by rewriting the effected bundles. Since all used chunks in the
bundle need to be written into new bundles and the reclaimed space depends on
the amount of unused chunks, only bundles with a high ratio of unused chunks
should be rewritten.
