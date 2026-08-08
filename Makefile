# Build, test and publish both the TypeScript (ts/) and Go (go/)
# implementations. ts/ is canonical; go/ tracks it.
#
# TypeScript resolves the engine via the node_modules symlink to the
# sibling ../parser/ts (wired by admin/scripts/link.sh).
#
# Go uses GOWORK=off deliberately: go/go.mod carries no `replace`, so this
# pins the engine to the PUBLISHED version in go.mod. Without it, the
# repo-set ../go.work (which lists ./debug/go) resolves the sibling
# ../parser/go instead.
#
# Note that CI resolves the other way: polyglot-ci.yml clones the siblings
# and generates a go.work over every module without a ../vendor/ replace
# (this one included), then runs a plain `go test` -- so CI builds against
# parser MAIN. Both resolutions must pass; run each before pushing.

.PHONY: all build test clean build-ts build-go test-ts test-go \
        clean-ts clean-go publish-ts publish-go tags-go reset

all: build test

build: build-ts build-go

test: test-ts test-go

clean: clean-ts clean-go

# --- TypeScript (package in ts/) ---
build-ts:
	cd ts && npm run build

test-ts:
	cd ts && npm test

clean-ts:
	rm -rf ts/dist ts/dist-test

# Publish the TypeScript package at its current package.json version.
publish-ts: test-ts
	cd ts && npm publish --access public

# --- Go (module in go/) ---
build-go:
	cd go && GOWORK=off go build ./...

test-go:
	cd go && GOWORK=off go test -v ./...

clean-go:
	cd go && GOWORK=off go clean

# Publish the Go module: make publish-go V=x.y.z
# Injects V into the Go `VERSION` const, commits, tags go/vX.Y.Z, and
# (when gh is available) creates a GitHub release.
publish-go: test-go
	@test -n "$(V)" || (echo "Usage: make publish-go V=x.y.z" && exit 1)
	sed -i.bak 's/^const VERSION = ".*"/const VERSION = "$(V)"/' go/debug.go
	rm -f go/debug.go.bak
	git add go/debug.go
	git commit -m "go: v$(V)"
	git tag go/v$(V)
	git push origin main go/v$(V)
	@command -v gh >/dev/null 2>&1 && gh release create go/v$(V) --title "go/v$(V)" --notes "Go module release v$(V)" || true

# List published Go module tags, newest first.
tags-go:
	git tag -l 'go/v*' --sort=-version:refname

reset:
	cd ts && npm run reset
	cd go && GOWORK=off go clean -cache && GOWORK=off go build ./... && GOWORK=off go test -v ./...
