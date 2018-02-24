MO_FILES = de.mo

default: default.pot ${MO_FILES}

default.pot: excluded.po ../src
	(cd ../src; find . -name '*.rs') | xargs xgettext -D ../src --debug -L python -n -F -a -E --from-code UTF-8 -x ../lang/excluded.po -o default.pot

%.mo : %.po
	msgfmt $< -o $@
