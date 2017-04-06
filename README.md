# zVault Backup Solution
zVault is a highly efficient deduplicating backup solution that supports
client-side encryption, compression and remote storage of backup data.

## Main Features

### Space efficient storage
Each file is split into a number of chunks. Content-defined chunking and chunk
fingerprints make sure that each chunk is only stored once. The chunking
algorithm is designed so that small changes to a file only change a few chunks
and leave most chunks unchanged. Multiple backups of the same data set will only
take up the space of one copy.

The deduplication in zVault is able to reuse existing data no matter whether a
file is modified, stored again under a different name, renamed or moved to
different folder.

That makes it possible to store daily backups without much overhead as backups
with only small changes do not take up much space.

Also multiple machines can share the same remote backup location and reuse the
data of each others for deduplication.

### Performance
High backup speed is a major design goal of zVault. Therefore is uses different
techniques to reach extremely fast backup speeds.

All used algorithms are hand-selected and optimized for speed.

Unmodified files are detected by comparing them to the last backup which makes
it possible to skip most of the files in regular usage.

A blazingly fast memory-mapped hash table tracks the fingerprints of all known
chunks so that chunks that are already in the repository can be skipped quickly.

In a general use case with a Linux system and a home folder of 50 GiB, backup
runs usually take between 1 and 2 minutes.

### Independent backups
All backups share common data in form of chunks but are independent on a higher
level. Backups can be deleted and chunks that are not used by any backup can be
removed.

Other backup solutions use differential backups organized in chains. This makes
those backups dependent on previous backups in the chain, so that those backups
can not be deleted. Also, restoring chained backups is much less efficient.

### Data encryption
The backup data can be protected by modern and fast encryption methods on the
client before storing it remotely.

### Compression
The backup data can be compressed to save even more space than by deduplication
alone. Users can choose between zlib (medium speed and compression),
lz4 (very fast, lower compression), brotli (medium speed, good compression), and
lzma (quite slow but amazing compression).

### Remote backup storage
zVault supports off-site backups via mounted filesystems. Backups can be stored
on any remote storage that can be mounted as a filesystem:
- NFS
- SMB / Windows shares
- SSH (via sshfs)
- FTP (via curlftpfs)
- Google Drive (via rclone)
- Amazon S3 (via rclone)
- Openstack Swift / Rackspace cloud files / Memset Memstore (via rclone)
- Dropbox (via rclone)
- Google Cloud Storage (via rclone)
- Amazon Drive (via rclone)
- Microsoft OneDrive (via rclone)
- Hubic (via rclone)
- Backblaze B2 (via rclone)
- Yandex Disk (via rclone)
- ... (potentially many more)

### Backup verification
For long-term storage of backups it is important to check backups regularly.
zVault offers a simple way to verify the integrity of backups.

### Mount backups as filesystems
Backups can be mounted as a user-space filesystem to investigate and restore
their contents. Once mounted, graphical programs like file managers can be used
to work on the backup data and find the needed files.


## Example usage

As an example, I am going to backup my projects folder. To do that, I am
initializing an encrypted zVault repository, storing the data on a remote
filesystem which has been mounted on `/mnt/backup`.

    #$> zvault init --encrypt --remote /mnt/backup
    public: 2bea1d15...
    secret: 3698a88c...

    Bundle size: 25.0 MiB
    Chunker: fastcdc/16
    Compression: brotli/3
    Encryption: 2bea1d15...
    Hash method: blake2

The repository has been created and zVault has generated as new key pair for me.
I should now store this key pair in a safe location before I continue.

Now I can backup my home directory to the repository.

    #$> zvault backup /home/dswd/projects ::projects1
    info: No reference backup found, doing a full scan instead
    Date: Thu,  6 Apr 2017 12:29:52 +0200
    Source: capanord:/home/dswd/projects
    Duration: 0:01:59.5
    Entries: 29205 files, 9535 dirs
    Total backup size: 5.4 GiB
    Modified data size: 5.4 GiB
    Deduplicated size: 3.2 GiB, 41.8% saved
    Compressed size: 1.1 GiB in 48 bundles, 63.9% saved
    Chunk count: 220410, avg size: 15.0 KiB

The backup run took about 2 minutes and by looking at the data, I see that
deduplication saved over 40% and compression again saved over 60% so that in the
end my backup only uses 1.1 GiB out of 5.4 GiB.

After some work, I create another backup.

    #$> zvault backup /home/dswd/projects ::projects2
    info: Using backup projects1 as reference
    Date: Thu,  6 Apr 2017 13:28:54 +0200
    Source: capanord:/home/dswd/projects
    Duration: 0:00:07.9
    Entries: 29205 files, 9535 dirs
    Total backup size: 5.4 GiB
    Modified data size: 24.9 MiB
    Deduplicated size: 10.6 MiB, 57.3% saved
    Compressed size: 4.7 MiB in 2 bundles, 55.7% saved
    Chunk count: 35507, avg size: 313 Bytes

This time, the backup run only took about 8 seconds as zVault skipped most of
the folder because it was unchanged. The backup only stored 4.7 MiB of data.
This shows the true potential of deduplication.


### Semantic Versioning
zVault sticks to the semantic versioning scheme. In its current pre-1.0 stage
this has the following implications:
- Even now the repository format is considered pretty stable. All future
  versions will be able to read the current repository format. Maybe conversions
  might be necessary but the backups should always be forward-compatible.
- The CLI might see breaking changes but at least it is guaranteed that calls
  that are currently non-destructive will not become destructive in the future.
  Running todays commands on a future version will not cause any harm.
