[private]
default:
	@ just --list --unsorted

lint:
	cargo fmt --all

test:
	cargo test \
		--all --quiet --message-format short \
		| egrep -v '^running [0-9]+ test' \
		| egrep -v '^test result' \
		| egrep -v '^$'

build:
	cargo build --release