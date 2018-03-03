set -ex

rm -rf repos
mkdir repos
mkdir -p repos/remotes/zvault_brotli3 repos/remotes/zvault_brotli6 repos/remotes/zvault_lzma2
target/release/zvault init --compression brotli/3 --remote $(pwd)/repos/remotes/zvault_brotli3 $(pwd)/repos/zvault_brotli3
target/release/zvault init --compression brotli/6 --remote $(pwd)/repos/remotes/zvault_brotli6 $(pwd)/repos/zvault_brotli6
target/release/zvault init --compression lzma2/2 --remote $(pwd)/repos/remotes/zvault_lzma2 $(pwd)/repos/zvault_lzma2
attic init repos/attic
borg init -e none repos/borg
borg init -e none repos/borg-zlib
zbackup init --non-encrypted repos/zbackup

cat < test_data/silesia.tar > /dev/null
time target/release/zvault backup test_data/silesia.tar $(pwd)/repos/zvault_brotli3::silesia1
time target/release/zvault backup test_data/silesia.tar $(pwd)/repos/zvault_brotli3::silesia2
time target/release/zvault backup test_data/silesia.tar $(pwd)/repos/zvault_brotli6::silesia1
time target/release/zvault backup test_data/silesia.tar $(pwd)/repos/zvault_brotli6::silesia2
time target/release/zvault backup test_data/silesia.tar $(pwd)/repos/zvault_lzma2::silesia1
time target/release/zvault backup test_data/silesia.tar $(pwd)/repos/zvault_lzma2::silesia2
time attic create repos/attic::silesia1 test_data/silesia.tar
time attic create repos/attic::silesia2 test_data/silesia.tar
time borg create -C none repos/borg::silesia1 test_data/silesia.tar
time borg create -C none repos/borg::silesia2 test_data/silesia.tar
time borg create -C zlib repos/borg-zlib::silesia1 test_data/silesia.tar
time borg create -C zlib repos/borg-zlib::silesia2 test_data/silesia.tar
time zbackup backup --non-encrypted repos/zbackup/backups/silesia1 < test_data/silesia.tar 
time zbackup backup --non-encrypted repos/zbackup/backups/silesia2 < test_data/silesia.tar 

du -h test_data/silesia.tar
du -sh repos/zvault*/bundles repos/attic repos/borg repos/borg-zlib repos/zbackup

rm -rf repos
mkdir repos
mkdir -p repos/remotes/zvault_brotli3 repos/remotes/zvault_brotli6 repos/remotes/zvault_lzma2
target/release/zvault init --compression brotli/3 --remote $(pwd)/repos/remotes/zvault_brotli3 $(pwd)/repos/zvault_brotli3
target/release/zvault init --compression brotli/6 --remote $(pwd)/repos/remotes/zvault_brotli6 $(pwd)/repos/zvault_brotli6
target/release/zvault init --compression lzma2/2 --remote $(pwd)/repos/remotes/zvault_lzma2 $(pwd)/repos/zvault_lzma2
attic init repos/attic
borg init -e none repos/borg
borg init -e none repos/borg-zlib
zbackup init --non-encrypted repos/zbackup

cat < test_data/ubuntu.tar > /dev/null
time target/release/zvault backup test_data/ubuntu.tar $(pwd)/repos/zvault_brotli3::ubuntu1
time target/release/zvault backup test_data/ubuntu.tar $(pwd)/repos/zvault_brotli3::ubuntu2
time target/release/zvault backup test_data/ubuntu.tar $(pwd)/repos/zvault_brotli6::ubuntu1
time target/release/zvault backup test_data/ubuntu.tar $(pwd)/repos/zvault_brotli6::ubuntu2
time target/release/zvault backup test_data/ubuntu.tar $(pwd)/repos/zvault_lzma2::ubuntu1
time target/release/zvault backup test_data/ubuntu.tar $(pwd)/repos/zvault_lzma2::ubuntu2
time attic create repos/attic::ubuntu1 test_data/ubuntu.tar
time attic create repos/attic::ubuntu2 test_data/ubuntu.tar
time borg create -C none repos/borg::ubuntu1 test_data/ubuntu.tar
time borg create -C none repos/borg::ubuntu2 test_data/ubuntu.tar
time borg create -C zlib repos/borg-zlib::ubuntu1 test_data/ubuntu.tar
time borg create -C zlib repos/borg-zlib::ubuntu2 test_data/ubuntu.tar
time zbackup backup --non-encrypted repos/zbackup/backups/ubuntu1 < test_data/ubuntu.tar 
time zbackup backup --non-encrypted repos/zbackup/backups/ubuntu2 < test_data/ubuntu.tar 

du -h test_data/ubuntu.tar
du -sh repos/zvault*/bundles repos/attic repos/borg repos/borg-zlib repos/zbackup
