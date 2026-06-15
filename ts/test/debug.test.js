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
    const am = new Tabnas()
    am.use(Debug, { print: false, trace: false })
    assert.equal(typeof am.debug.describe, 'function')
    assert.equal(typeof am.debug.describe(), 'string')

    const out = am.debug.describe()
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
    const am = new Tabnas({ tag: 'demo' })
    am.use(Debug, { print: false, trace: false })
    const out = am.debug.describe()
    assert.ok(out.includes('tag: demo'), 'describe() should report the instance tag')
    assert.ok(out.includes('  start: '), 'describe() should report the rule start')
  })
})
