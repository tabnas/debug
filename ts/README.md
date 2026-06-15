# @tabnas/debug

Debug plugin for the [`tabnas`](https://github.com/rjrodger/tabnas) parser.

Adds tracing helpers and a `describe()` method to a `Tabnas` instance.

## Install

```bash
npm install tabnas @tabnas/debug
```

## Use

```js
const { Tabnas } = require('tabnas')
const { Debug } = require('@tabnas/debug')

const tn = new Tabnas({ plugins: [Debug] })
console.log(tn.debug.describe())
```

## License

MIT.
