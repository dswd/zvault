% Bundle file format
## Bundle file format

The bundle file format consists of 4 parts:
- A magic header with version
- An encoded header structure
- An encoded chunk list
- The chunk data

The main reason for having those multiple parts is that it is expected that the
smaller front parts can be read much faster than the the whole file. So
information that is needed more frequently is put into earlier parts and the
data that is need the least frequent is put into the latter part so that it does
not slow down reading the front parts. Keeping those parts in separate files
was also considered but rejected to increase the reliability of the storage.


### Magic header with version
The first part of a bundle file contains an 8 byte magic header with version
information.

The first 6 bytes of the header consist of the fixed string "zvault", followed
by one byte with the fixed value 0x01. Those 7 bytes make up the magic header of
the file and serve to identify the file type as a zvault bundle file.

The 8th byte of the first file part is the version of the file format. This
value is currently 0x01 and is expected to be increased for any breaking changes
in the file format.


### Encoded header structure
The encoded header structure is the second part of the bundle file format and
follows directly after the 8 bytes of the magic header.

The header structure is defined in `bundle.rs` as `BundleInfo` and contains
general information on the bundle's contents and on how to decode the other two
parts of the bundle file.

This header structure is encoded using the *MsgPack* format. It is neither
compressed (since its size is pretty small) nor encrypted (since it only
contains general information and no user data) in any way.


### Encoded chunk list
The chunk list is the third part of the bundle file and follows directly after
the encoded header structure.

The chunk list contains hashes and sizes of all chunks stored in this bundle in
the order they are stored. The list is encoded efficiently as 20 bytes per chunk
(16 for the hash and 4 for the size) as defined in `../util/chunk.rs`.

Since the chunk list contains confidential information (the chunk hashes and
sized can be used to identify files) the encoded chunk list is encrypted using
the encryption method specified in the header structure. The header structure
also contains the full size of the encoded and encrypted chunk list which is
needed since the encryption could add some bytes for a nonce or an
authentication code.

The chunk list is not compressed since the hashes have a very high entropy and
do not compress significantly.

The chunk list is not stored in the header structure because it contains
confidential data and the encryption method is stored in the header. Also the
chunk list can be pretty big compared to the header which needs to be read more
often.


### Chunk data
The chunk data is the final part of a bundle file and follows after the encoded
chunk list. The starting position can be obtained from the header as the encoded
size of the chunk list is stored there.

The chunk data part consists of the content data of the chunks contained in this
bundle simply concatenated without any separator. The actual size (and by
summing up the sizes also the starting position) of each chunk can be obtained
from the chunk list.

The chunk data is compressed as whole (solid archive) and encrypted with the
methods specified in the bundle header structure.
