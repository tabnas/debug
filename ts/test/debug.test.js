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
  })
})
