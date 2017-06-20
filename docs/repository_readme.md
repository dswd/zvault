# ZVault repository

This folder is a zVault remote repository and contains backup data.

The repository contains the following components:
* The backup bundles in the subfolder `bundles`. The individual files are
  organized in subfolders and named after their bundle ids. The structure and
  names of the files is not important as the files include the bundle id in
  their headers. Thus the files can be renamed and reorganized.
* The backup anchor files in the subfolder `backups`. The names of the files
  and their structure determine the backup names but are not used otherwise.
* Active locks in the subfolder `locks`. This folder only contains lock files
  when the repository is currently used. If any zVault process crashes, a stale
  lock file might be left back. Those files can be safely removed if no process
  is running for sure.





## Repository format

In case the zVault software is not available for restoring the backups included
in this repository the following sections describe the format of the repository
so that its contents can be read without zVault.


### Bundle files
The bundle file format consists of 5 parts:
- A magic header with version
- A tiny header with encryption information
- An encoded and encrypted bundle information structure
- An encoded and encrypted chunk list
- The chunk data (compressed and encrypted)

The main reason for having those multiple parts is that it is expected that the
smaller front parts can be read much faster than the the whole file. So
information that is needed more frequently is put into earlier parts and the
data that is need the least frequent is put into the latter part so that it does
not slow down reading the front parts. Keeping those parts in separate files
was also considered but rejected to increase the reliability of the storage.


#### Magic header with version
The first part of a bundle file contains an 8 byte magic header with version
information.

The first 6 bytes of the header consist of the fixed string "zvault", followed
by one byte with the fixed value 0x01. Those 7 bytes make up the magic header of
the file and serve to identify the file type as a zvault bundle file.

The 8th byte of the first file part is the version of the file format. This
value is currently 0x01 and is expected to be increased for any breaking changes
in the file format.


#### Encryption header
The encryption header is the second part of the bundle file format and follows
directly after the 8 bytes of the magic header.

The header structure is defined in the appendix as `BundleHeader` and contains
information on how to decrypt the other parts of the bundle as well as the
encrypted size of the following bundle information.

Please note that this header even exists when the bundle is not encrypted (the
header then contains no encryption method).


#### Bundle information
The bundle information structure is the third part of the bundle file format and
follows directly after the encryption header.

The information structure is defined in the appendix as `BundleInfo` and
contains general information on the bundle's contents and on how to decode the
other two parts of the bundle file.

This structure is encrypted using the method described in the previous
encryption header since it contains confidential information (the bundle id
could be used to identify the data contained in the bundle). The size of the
encrypted structure is also stored in the previous header. This structure is not
compressed, as it is pretty small.


#### Encoded chunk list
The chunk list is the forth part of the bundle file and follows directly after
the bundle information structure.

The chunk list contains hashes and sizes of all chunks stored in this bundle in
the order they are stored. The list is encoded as defined in the appendix as
`ChunkList`.

Since the chunk list contains confidential information (the chunk hashes and
sized can be used to identify files) the encoded chunk list is encrypted using
the encryption method specified in the encryption header. The bundle information
structure contains the full size of the encoded and encrypted chunk list as
`chunk_list_size` which is needed since the encryption could add some bytes for
a nonce or an authentication code.

The chunk list is not compressed since the hashes have a very high entropy and
do not compress significantly.

The chunk list is not stored in the bundle info structure because it can be
pretty big compared to the info structure which needs to be read more often.


#### Chunk data
The chunk data is the final part of a bundle file and follows after the chunk
list. The starting position can be obtained from the bundle info structure as
the encoded size of the chunk list is stored there as `chunk_list_size`.

The chunk data part consists of the data of the chunks contained in this
bundle simply concatenated without any separator. The individual chunk sizes can
be obtained from the chunk list. The starting position of any chunk can be
calculated by summing up the sized of all previous chunks.

The chunk data is compressed as whole (solid archive) and encrypted with the
methods specified in the bundle information structure.


### Backup format
The repository contains multiple backups that share the data contained in the
bundles. The individual backups are encoded in backup files as described in the
following section. Those backup files reference a list of chunks in the bundles
as a root inode entry. Each inode entry references lists of chunks for its data
and potential child entries.

All chunks that are referenced either in the backup files or in the inode
entries are contained in one of the bundles and is uniquely identified by its
hash. An index, e.g. a hash table, can help to find the correct bundle quickly.


#### Backup files
Backup files contain information on one specific backup and reference the
directory root of that backup.

Backup files consist of 3 parts:
- A magic header with version
- A tiny header with encryption information
- An encoded and encrypted backup information structure


##### Magic header with version
The first part of a backup file contains an 8 byte magic header with version
information.

The first 6 bytes of the header consist of the fixed string "zvault", followed
by one byte with the fixed value 0x03. Those 7 bytes make up the magic header of
the file and serve to identify the file type as a zvault backup file.

The 8th byte of the first file part is the version of the file format. This
value is currently 0x01 and is expected to be increased for any breaking changes
in the file format.


##### Encryption header
The encryption header is the second part of the backup file format and follows
directly after the 8 bytes of the magic header.

The header structure is defined in the appendix as `BackupHeader` and contains
information on how to decrypt the rest of the backup file.

Please note that this header even exists when the backup file is not encrypted
(the header then contains no encryption method).


##### Backup information
The backup information structure is the final part of the backup file format and
follows directly after the encryption header.

The information structure is defined in the appendix as `Backup` and
contains general information on the backup's contents and references the
directory root of the backup tree.

This structure is encrypted using the method described in the previous
encryption header since it contains confidential information. This structure is
not compressed, as it is pretty small.


#### Directories & file data
The inode entries are encoded as defined in the appendix as `Inode`. The inode
structure contains all meta information on an inode entry, e.g. its file type,
the data size, modification time, permissions and ownership, etc. Also, the
structure contains optional information that is specific to the file type.
For regular files, the inode structure contains the data of that file either
inline (for very small files) or as a reference via a chunk list.
For directories, the inode structure contains a mapping of child inode entries
with their name as key and a chunk list referring their encoded `Inode`
structure as value.
For symlinks, the inode structure contains the target in the field
`symlink_target`.

Starting from the `root` of the `Backup` structure, the whole backup file tree
can be reconstructed by traversing the children of each inode recursively.
Since files can only be retrieved by traversing their parent directories, they
contain no back link to their parent directory.





## Appendix

### MessagePack encoding

Most zvault structures are encoded using the MessagePack encoding as specified
at http://www.msgpack.org. The version of MessagePack that is used, is dated to
2013-04-21.

All structure encodings are based on a mapping that associates values to the
structure's fields. In order to save space, the structure's fields are not
referenced by name but by an assigned number. In the encoding specification,
this is written as `FIELD: TYPE => NUMBER` where `FIELD` is the field name used
to reference the field in the rest of the description, `TYPE` is the type of the
field's values and `NUMBER` is the number used as key for this field in the
mapping.

The simple types used are called `null`, `bool`, `int`, `float`, `string`
and `bytes` that correspond to the MessagePack data types (`null` means `Nil`,
`bytes` means `Binary` and the other types are lower case to distinguish them
from custom types).

Complex data types are noted as `{KEY => VALUE}` for mappings and `[TYPE]`
for arrays. Tuples of multiple types e.g. `(TYPE1, TYPE2, TYPE3)` are also
encoded as arrays but regarded as differently as they contain different types
and have a fixed length.

If a field is optional, its type is listed as `TYPE?` which means that
either `null` or the `TYPE` is expected. If a value of `TYPE` is given. the
option is regarded as being set and if `null` is given, the option is regarded
as not being set.

If a structure contains fields with structures or other complex data types, the
values of those fields are encoded as described for those values (often again as
a mapping on their own). The encoding specification uses the name of the
structure as a field type in this case.

For some structures, there exist a set of default values for the structure's
fields. If any field is missing in the encoded mapping, the corresponding value
from the defaults will be taken instead.


### Constants
The following types are used as named constants. In the encoding, simply the
value (mostly a number) is used instead of the name but in the rest of the
specification the name is used for clarity.


#### `BundleMode`
The `BundleMode` describes the contents of the chunks of a bundle.
- `Data` means that the chunks contain file data
- `Meta` means that the chunks either contain encoded chunk lists or encoded
  inode metadata

    BundleMode {
        Data => 0,
        Meta => 1
    }


#### `HashMethod`
The `HashMethod` describes the method used to create fingerprint hashes from
chunk data. This is not relevant for reading backups.
- `Blake2` means the hash method `Blake2b` as described in RFC 7693 with the
  hash length set to 128 bits.
- `Murmur3` means the hash method `MurmurHash3` as described at
  https://en.wikipedia.org/wiki/MurmurHash for the x64 architecture and with the
  hash length set to 128 bits.

    HashMethod {
        Blake2 => 1,
        Murmur3 => 2
    }


#### `EncryptionMethod`
The `EncryptionMethod` describes the method used to encrypt (and thus also
decrypt) data.
- `Sodium` means the `crypto_box_seal` method of `libsodium` as specified at
  http://www.libsodium.org as a combination of `X25519` and `XSalsa20-Poly1305`.

    EncryptionMethod {
        Sodium => 0
    }


#### `CompressionMethod`
The `CompressionMethod` describes a compression method used to compress (and
thus also decompress) data.
- `Deflate` means the gzip/zlib method (without header) as described in RFC 1951
- `Brotli` means the Google Brotli method as described in RFC 7932
- `Lzma` means the LZMA method (XZ stream format) as described at
  http://tukaani.org/xz/
- `Lz4` means the LZ4 method as described at http://www.lz4.org

    CompressionMethod {
        Deflate => 0,
        Brotli => 1,
        Lzma => 2,
        Lz4 => 3
    }


#### `FileType`
The `FileType` describes the type of an inode.
- `File` means on ordinary file that contains data
- `Directory` means a directory that does not contain data but might have
  children
- `Symlink` means a symlink that points to a target
- `BlockDevice` means a block device
- `CharDevice` means a character device
- `NamedPipe` means a named pipe/fifo

    FileType {
        File => 0,
        Directory => 1,
        Symlink => 2,
        BlockDevice => 3,
        CharDevice => 4,
        NamedPipe => 5
    }


### Types
The following types are used to simplify the encoding specifications. They can
simply be substituted by their definitions. For simplicity, their names will be
used in the encoding specifications instead of their definitions.


#### `Encryption`
The `Encryption` is a combination of an `EncryptionMethod` and a key.
The method specifies how the key was used to encrypt the data.
For the `Sodium` method, the key is the public key used to encrypt the data
with. The secret key needed for decryption, must correspond to that public key.

    Encryption = (EncryptionMethod, bytes)


#### `Compression`
The `Compression` is a micro-structure containing the compression method and the
compression level. The level is only used for compression.

    Compression {
        method: CompressionMethod => 0,
        level: int => 1
    }


### `BundleHeader` encoding
The `BundleHeader` structure contains information on how to decrypt other parts
of a bundle. The structure is encoded using the MessagePack encoding that has
been defined in a previous section.
The `encryption` field contains the information needed to decrypt the rest of
the bundle parts. If the `encryption` option is set, the following parts are
encrypted using the specified method and key, otherwise the parts are not
encrypted. The `info_size` contains the encrypted size of the following
`BundleInfo` structure.

    BundleHeader {
        encryption: Encryption? => 0,
        info_size: int => 1
    }


### `BundeInfo` encoding
The `BundleInfo` structure contains information on a bundle. The structure is
encoded using the MessagePack encoding that has been defined in a previous
section.
If the `compression` option is set, the chunk data is compressed with the
specified method, otherwise it is uncompressed. The encrypted size of the
following `ChunkList` is stored in the `chunk_list_size` field.

    BundeInfo {
        id: bytes => 0,
        mode: BundleMode => 1,
        compression: Compression? => 2,
        hash_method: HashMethod => 4,
        raw_size: int => 6,
        encoded_size: int => 7,
        chunk_count: int => 8,
        chunk_list_size: int => 9
    }

This structure is encoded with the following field default values:
- `hash_method`: `Blake2`
- `mode`: `Data`
- All other fields: `0`, `null` or an empty byte sequence depending on the type.


### `ChunkList` encoding
The `ChunkList` contains a list of chunk hashes and chunk sizes. This list is
NOT encoded using the MessagePack format as a simple binary format is much more
efficient in this case.

For each chunk, the hash and its size are encoded in the following way:
- The hash is encoded as 16 bytes (little-endian).
- The size is encoded as a 32-bit value (4 bytes) in little-endian.
The encoded hash and the size are concatenated (hash first, size second)
yielding 20 bytes for each chunk.
Those 20 bytes of encoded chunk information are concatenated for all chunks in
the list in order or appearance in the list.


### `Inode` encoding
The `Inode` structure contains information on a backup inode, e.g. a file or
a directory. The structure is encoded using the MessagePack encoding that has
been defined in a previous section.
The `name` field contains the name of this inode which can be concatenated with
the names of all parent inodes (with a platform-dependent seperator) to form the
full path of the inode.
The `size` field contains the raw size of the data in
bytes (this is 0 for everything except files).
The `file_type` specifies the type of this inode.
The `mode` field specifies the permissions of the inode as a number which is
normally interpreted as octal.
The `user` and `group` fields specify the ownership of the inode in the form of
user and group id.
The `timestamp` specifies the modification time of the inode in whole seconds
since the UNIX epoch (1970-01-01 12:00 am).
The `symlink_target` specifies the target of symlink inodes and is only set for
symlinks.
The `data` specifies the data of a file and is only set for regular files. The
data is specified as a tuple of `nesting` and `bytes`. If `nesting` is `0`,
`bytes` contains the data of the file. This "inline" format is only used for
small files. If `nesting` is `1`, `bytes` is an encoded `ChunkList` (as
described in a previous section). The concatenated data of those chunks make up
the data of the file. If `nesting` is `2`, `bytes` is also an encoded
`ChunkList`, but the concatenated data of those chunks form again an encoded
`ChunkList` which in turn contains the chunks with the file data. Thus `nesting`
specifies the number of indirection steps via `ChunkList`s.
The `children` field specifies the child inodes of a directory and is only set
for directories. It is a mapping from the name of the child entry to the bytes
of the encoded chunklist of the encoded `Inode` structure of the child. It is
important that the names in the mapping correspond with the names in the
respective child `Inode`s and that the mapping is stored in alphabetic order of
the names.
The `cum_size`, `cum_dirs` and `cum_files` are cumulative values for the inode
as well as the whole subtree (including all children recursively). `cum_size` is
the sum of all inode data sizes plus 1000 bytes for each inode (for encoded
metadata). `cum_dirs` and `cum_files` is the count of directories and
non-directories (symlinks and regular files).
The `xattrs` contains a mapping of all extended attributes of the inode. And
`device` contains a tuple with the major and minor device id if the inode is a
block or character device.

    Inode {
        name: string => 0,
        size: int => 1,
        file_type: FileType => 2,
        mode: int => 3,
        user: int => 4,
        group: int => 5,
        timestamp: int => 7,
        symlink_target: string? => 9,
        data: (int, bytes)? => 10,
        children: {string => bytes}? => 11,
        cum_size: int => 12,
        cum_dirs: int => 13,
        cum_files: int => 14
        xattrs: {string => bytes}? => 15,
        device: (int, int)? => 16
    }

This structure is encoded with the following field default values:
- `file_type`: `File`
- `mode`: `0o644`
- `user` and `group`: `1000`
- All other fields: `0`, `null` or an empty string depending on the type.


### `BackupHeader` encoding
The `BackupHeader` structure contains information on how to decrypt the rest of
the backup file. The structure is encoded using the MessagePack encoding that
has been defined in a previous section.
The `encryption` field contains the information needed to decrypt the rest of
the backup file. If the `encryption` option is set, the rest of the backup file
is encrypted using the specified method and key, otherwise the rest is not
encrypted.

    BackupHeader {
        encryption: Encryption? => 0
    }


### `Backup` encoding
The `Backup` structure contains information on one specific backup and
references the root of the backup file tree. The structure is encoded using the
MessagePack encoding that has been defined in a previous section.
The `root` field contains an encoded `ChunkList` that references the root of the
backup file tree.
The fields `total_data_size`, `changed_data_size`, `deduplicated_data_size` and
`encoded_data_size` list the sizes of the backup in various stages in bytes.
- `total_data_size` gives the cumulative sizes of all entries in the backup.
- `changed_data_size` gives the size of only those entries that changed since
  the reference backup.
- `deduplicated_data_size` gives the cumulative raw size of all new chunks in
  this backup that have not been stored in the repository yet.
- `encoded_data_size` gives the cumulative encoded (and compressed) size of all
  new bundles that have been written specifically to store this backup.
The fields `bundle_count` and `chunk_count` contain the number of new bundles
and chunks that had to be written to store this backup. `avg_chunk_size` is the
average size of new chunks in this backup.
The field `date` specifies the start of the backup run in seconds since the UNIX
epoch and the field `duration` contains the duration of the backup run in
seconds as a floating point number containing also fractions of seconds.
The fields `file_count` and `dir_count` contain the total number of
non-directories and directories in this backup.
The `host` and `path` field contain the host name and the the path on that host
where the root of the backup was located.
The field `config` contains the configuration of zVault during the backup run.

    Backup {
        root: bytes => 0,
        total_data_size: int => 1,
        changed_data_size: int => 2,
        deduplicated_data_size: int => 3,
        encoded_data_size: int => 4,
        bundle_count: int => 5,
        chunk_count: int => 6,
        avg_chunk_size: float => 7,
        date: int => 8,
        duration: float => 9,
        file_count: int => 10,
        dir_count: int => 11,
        host: string => 12,
        path: string => 13,
        config: Config => 14
    }
