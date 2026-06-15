/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */
'use strict'

const { describe, it } = require('node:test')
const assert = require('node:assert')

const { Tabnas } = require('tabnas')
const { Debug } = require('..')

describe('debug', () => {
  it('loads', () => {
    assert.ok(Debug != null)
  })

  it('decorates an instance with describe()', () => {
    const tn = new Tabnas()
    tn.use(Debug, { print: false, trace: false })
    assert.equal(typeof tn.debug.describe, 'function')
    assert.equal(typeof tn.debug.describe(), 'string')

    const out = tn.debug.describe()
    for (const header of [
      '========= INSTANCE ========',
      '========= TOKENS ========',
      '========= RULES =========',
      '========= ALTS =========',
      '========= LEXER =========',
      '========= CONFIG ========',
      '========= PLUGIN =========',
    ]) {
      assert.ok(out.includes(header), 'describe() missing section ' + header)
    }
  })

  it('reports the instance tag and config in describe()', () => {
    const tn = new Tabnas({ tag: 'demo' })
    tn.use(Debug, { print: false, trace: false })
    const out = tn.debug.describe()
    assert.ok(out.includes('tag: demo'), 'describe() should report the instance tag')
    assert.ok(out.includes('  start: '), 'describe() should report the rule start')
  })
})
