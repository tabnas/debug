// Copyright (c) 2026 Richard Rodger and other contributors, MIT License

package tabnasdebug_test

// Named grammar fixtures shared by the spec runner and the in-language
// tests. Their TypeScript counterparts are in `ts/test/fixture.js` — the two
// registries must stay in step, because `test/spec/*.tsv` addresses a
// grammar by NAME and both runtimes must build the same one.
//
// The grammars are hand-written against the engine on purpose: @tabnas/abnf
// must NOT become a dependency of @tabnas/debug (the emitter reads only the
// live engine), so no fixture may be compiled from ABNF source here.

import (
	tabnas "github.com/tabnas/parser/go"
)

// bareGrammar: the engine with nothing installed. Pins what Describe emits
// for an instance with no grammar at all.
func bareGrammar() *tabnas.Tabnas {
	return tabnas.Make()
}

// addGrammar: `val` pushes `add`; `add` matches #NR then optionally a
// #PL-replace back into `add`, with an epsilon close and the #ZZ end close.
// Exercises the emitter's optional folding (`[ PL add ]`) and token
// definitions.
func addGrammar() *tabnas.Tabnas {
	plus := "+"
	j := tabnas.Make(tabnas.Options{
		Fixed: &tabnas.FixedOptions{Token: map[string]*string{"#PL": &plus}},
		Rule:  &tabnas.RuleOptions{Start: "val"},
	})
	zz := j.Token("#ZZ")
	nr := j.Token("#NR")
	pl := j.Token("#PL")

	j.Rule("val", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{P: "add"})
	})
	j.Rule("add", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{S: [][]tabnas.Tin{{nr}}})
		rs.AddClose(&tabnas.AltSpec{S: [][]tabnas.Tin{{pl}}, R: "add"})
		rs.AddClose(&tabnas.AltSpec{})
		rs.AddClose(&tabnas.AltSpec{S: [][]tabnas.Tin{{zz}}})
	})
	return j
}

// greetGrammar: a two-way alternation over case-sensitive literals.
// Exercises the emitter's `/` alternation and %s"…" literal rendering.
func greetGrammar() *tabnas.Tabnas {
	hi, hello := "hi", "hello"
	j := tabnas.Make(tabnas.Options{
		Fixed: &tabnas.FixedOptions{
			Token: map[string]*string{"#HI": &hi, "#HE": &hello},
		},
		Rule: &tabnas.RuleOptions{Start: "greet"},
	})
	zz := j.Token("#ZZ")
	tHI := j.Token("#HI")
	tHE := j.Token("#HE")

	j.Rule("greet", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(
			&tabnas.AltSpec{S: [][]tabnas.Tin{{tHI}}},
			&tabnas.AltSpec{S: [][]tabnas.Tin{{tHE}}},
		)
		rs.AddClose(&tabnas.AltSpec{S: [][]tabnas.Tin{{zz}}})
	})
	return j
}

// grammars is the registry the spec fixtures address by name.
var grammars = map[string]func() *tabnas.Tabnas{
	"bare":  bareGrammar,
	"add":   addGrammar,
	"greet": greetGrammar,
}
