.PHONY: all build test clean parser build-ts test-ts build-go test-go

# The TypeScript implementation in ts/ is canonical. The Go
# implementation in go/ is kept at parity with it. `all` builds and
# tests both.
#
# Both implementations consume the tabnas parser engine from source. The
# `parser` target downloads and builds its GitHub main branch into
# vendor/ (git-ignored); build/test depend on it.

all: build test

parser:
	./scripts/fetch-parser.sh

build: build-ts build-go

test: test-ts test-go

clean:
	$(MAKE) -C ts clean
	$(MAKE) -C go clean
	rm -rf vendor

# TypeScript (canonical)
build-ts: parser
	cd ts && npm install && npm run build

test-ts: build-ts
	cd ts && npm test

# Go (parity)
build-go: parser
	cd go && go build ./...

test-go: build-go
	cd go && go test ./...
