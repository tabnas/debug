# @tabnas/debug

Debug plugin for the [`tabnas`](https://github.com/tabnas/parser) parser.

Adds tracing helpers and a `describe()` method to a `Tabnas` instance.

## Install

```bash
npm install @tabnas/parser @tabnas/debug
```

## Use

```js
const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('@tabnas/debug')

const tn = new Tabnas({ plugins: [Debug] })
console.log(tn.debug.describe())
```

## License

MIT.
