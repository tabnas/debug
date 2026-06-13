module github.com/tabnas/debug/go

go 1.24.7

require github.com/tabnas/parser/go v0.0.0

// The tabnas parser engine is consumed from source (it is not published
// to a module proxy). scripts/fetch-parser.sh downloads its GitHub main
// branch into ./vendor; this replace points the require there.
replace github.com/tabnas/parser/go => ../vendor/tabnas-parser/go
