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
file is modified, stored again under a different name, renamed or moved to a
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


## Example scenario

I am using zVault on several of my computers. Here are some numbers from my
desktop PC. On this computer I am running daily backups of both the system `/`
(excluding some folders like `/home`) with 12.9 GiB and the home folder `/home`
with 53.6 GiB.

    $> zvault config ::
    Bundle size: 25.0 MiB
    Chunker: fastcdc/16
    Compression: brotli/3
    Encryption: 8678d...
    Hash method: blake2

The backup repository uses the default configuration with encryption enabled.
The repository currently contains 12 backup versions of each folder. Both
folders combined currently contain over 66.5 GiB not counting changes between
the different versions.

    $> zvault info ::
    Bundles: 1675
    Total size: 37.9 GiB
    Uncompressed size: 58.1 GiB
    Compression ratio: 65.3%
    Chunk count: 5580237
    Average chunk size: 10.9 KiB
    Index: 192.0 MiB, 67% full

The repository info reveals that the data stored in the repository is only
58.1 GiB, so 8.4 GiB / 12.5% has been saved by deduplication. Another 20.2 GiB /
34.7% have been saved by compression. In total, 28.6 out of 66.5 GiB / 43% have
been saved.

The data is stored in over 5 million chunks of an average size of 10.9 KiB. The
average chunk is smaller than configured because of files smaller than the chunk
size. The chunks are stored in an index file which takes up 192 MiB on disk and
in memory during backup runs. Additionally, 337 MiB of bundle data is stored
locally to allow fast access to metadata. In total that is less than 1% of the
original data.

    $> zvault info ::home/2017-06-19
    Date: Mon, 19 Jun 2017 00:00:48 +0200
    Source: desktop:/home
    Duration: 0:01:57.2
    Entries: 193624 files, 40651 dirs
    Total backup size: 53.6 GiB
    Modified data size: 2.4 GiB
    Deduplicated size: 50.8 MiB, 97.9% saved
    Compressed size: 8.9 MiB in 2 bundles, 82.4% saved
    Chunk count: 2443, avg size: 21.3 KiB

This is the information on the last backup run for `/home`. The total data in
that backup is 53.6 GiB of which 2.4 GiB have been detected to have changed by
comparing file dates and sizes to the last backup. Of those changed files,
deduplication reduced the data to 50.8 MiB and compression reduced this to
8.9 MiB. The whole backup run took less than 2 minutes.

    $> zvault info ::system/2017-06-19
    Date: Mon, 19 Jun 2017 00:00:01 +0200
    Source: desktop:/
    Duration: 0:00:46.5
    Entries: 435905 files, 56257 dirs
    Total backup size: 12.9 GiB
    Modified data size: 43.1 MiB
    Deduplicated size: 6.8 MiB, 84.2% saved
    Compressed size: 1.9 MiB in 2 bundles, 72.3% saved
    Chunk count: 497, avg size: 14.0 KiB

The information of the last backup run for `/` looks similar. Out of 12.9 GiB,
deduplication and compression reduced the new data to 1.9 MiB and the backup
took less than one minute.

This data seems representative as other backup runs and other systems yield
similar results.


### Semantic Versioning
zVault sticks to the semantic versioning scheme. In its current pre-1.0 stage
this has the following implications:
- Even now the repository format is considered pretty stable. All future
  versions will be able to read the current repository format. Maybe conversions
  might be necessary but the backups should always be forward-compatible.
- The CLI might see breaking changes but at least it is guaranteed that calls
  that are currently non-destructive will not become destructive in the future.
  Running todays commands on a future version will not cause any harm.
