.PHONY: all build test clean build-ts test-ts build-go test-go

# The TypeScript implementation in ts/ is canonical. The Go
# implementation in go/ is kept at parity with it. `all` builds and
# tests both.

all: build test

build: build-ts build-go

test: test-ts test-go

clean:
	$(MAKE) -C ts clean
	$(MAKE) -C go clean

# TypeScript (canonical)
build-ts:
	$(MAKE) -C ts build

test-ts:
	$(MAKE) -C ts test

# Go (parity)
build-go:
	$(MAKE) -C go build

test-go:
	$(MAKE) -C go test
