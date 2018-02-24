PACKAGE=zvault
DEPENDENCIES=debhelper devscripts

.PHONY: default
default: clean build

.PHONY: build
build: libsquash_*.deb $(PACKAGE)_*.deb

libsquash_*.deb:
	(cd libsquash; make clean; debuild -b -us -uc; cd ..)

$(PACKAGE)_*.deb: $(PACKAGE)/man/* $(PACKAGE)/zvault
	(cd $(PACKAGE); make clean; debuild -b -us -uc; cd ..)

.PHONY: clean
clean:
	(cd $(PACKAGE); debuild clean; cd ..)
	rm -rf $(PACKAGE)_*
	(cd libsquash; debuild clean; cd ..)
	rm -rf libsqash_*

$(PACKAGE)/man/*: ../docs/man/*
	cp ../docs/man/* $(PACKAGE)/man

$(PACKAGE)/zvault: ../target/release/zvault
	cp ../target/release/zvault $(PACKAGE)/zvault
        strip -s $(PACKAGE)/zvault

../target/release/zvault: ../src/*.rs ../Cargo.toml
	(cd ..; cargo build --release)
