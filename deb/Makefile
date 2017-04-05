PACKAGE=zvault
DEPENDENCIES=debhelper devscripts

.PHONY: default
default: clean build

.PHONY: build
build: libsquash_*.deb $(PACKAGE)_*.deb

libsquash_*.deb:
	(cd libsquash; make clean; debuild -b -us -uc; cd ..)

$(PACKAGE)_*.deb: $(PACKAGE)/zvault.1.ronn $(PACKAGE)/zvault
	(cd $(PACKAGE); make clean; debuild -b -us -uc; cd ..)

.PHONY: clean
clean:
	(cd $(PACKAGE); debuild clean; cd ..)
	rm -rf $(PACKAGE)_*
	(cd libsquash; debuild clean; cd ..)
	rm -rf libsqash_*
	rm -f ../target/release/zvault

$(PACKAGE)/zvault.1.ronn: ../docs/manpage.md
	cp ../docs/manpage.md $(PACKAGE)/zvault.1.ronn

$(PACKAGE)/zvault: ../target/release/zvault
	cp ../target/release/zvault $(PACKAGE)/zvault

../target/release/zvault: ../src/*.rs ../Cargo.toml
	(cd ..; cargo build --release)
