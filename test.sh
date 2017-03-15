set -ex

mkdir repos
time target/release/zvault init --compression brotli/3 repos/zvault_brotli3
time target/release/zvault init --compression brotli/6 repos/zvault_brotli6
time target/release/zvault init --compression lzma2/2 repos/zvault_lzma2
time attic init repos/attic
time borg init -e none repos/borg
time borg init -e none repos/borg-zlib
time zbackup init --non-encrypted repos/zbackup

cat < test_data/silesia.tar > /dev/null
time target/release/zvault put repos/zvault_brotli3::silesia1 test_data/silesia.tar
time target/release/zvault put repos/zvault_brotli3::silesia2 test_data/silesia.tar
time target/release/zvault put repos/zvault_brotli6::silesia1 test_data/silesia.tar
time target/release/zvault put repos/zvault_brotli6::silesia2 test_data/silesia.tar
time target/release/zvault put repos/zvault_lzma2::silesia1 test_data/silesia.tar
time target/release/zvault put repos/zvault_lzma2::silesia2 test_data/silesia.tar
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
time target/release/zvault init --compression brotli/3 repos/zvault_brotli3
time target/release/zvault init --compression brotli/6 repos/zvault_brotli6
time target/release/zvault init --compression lzma2/2 repos/zvault_lzma2
time attic init repos/attic
time borg init -e none repos/borg
time borg init -e none repos/borg-zlib
time zbackup init --non-encrypted repos/zbackup

cat < test_data/ubuntu.tar > /dev/null
time target/release/zvault put repos/zvault_brotli3::ubuntu1 test_data/ubuntu.tar
time target/release/zvault put repos/zvault_brotli3::ubuntu2 test_data/ubuntu.tar
time target/release/zvault put repos/zvault_brotli6::ubuntu1 test_data/ubuntu.tar
time target/release/zvault put repos/zvault_brotli6::ubuntu2 test_data/ubuntu.tar
time target/release/zvault put repos/zvault_lzma2::ubuntu1 test_data/ubuntu.tar
time target/release/zvault put repos/zvault_lzma2::ubuntu2 test_data/ubuntu.tar
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
