MO_FILES = de.mo

default: default.pot ${MO_FILES}

default.pot: excluded.po ../src
	find ../src -name '*.rs' | xargs xgettext --debug -L python -n -F -a -E --from-code UTF-8 -x ../lang/excluded.po -o default.pot

%.mo : %.po
	msgfmt $< -o $@
