# left-pad

> String left pad

A minimal npm package that pads a string on the left side with a specified character until the string reaches a desired total length.

## Install

```sh
npm install left-pad
```

## Usage

```js
var leftPad = require('left-pad');

leftPad('foo', 5);        // '  foo'
leftPad('foo', 5, '0');   // '00foo'
leftPad(42, 5, '0');      // '00042'
leftPad('hello', 5);      // 'hello'  (no truncation)
```

## API

### `leftPad(str, len, [ch])`

**Parameters:**

- `str` *(string | number)* — The value to pad. Numbers are coerced to strings.
- `len` *(number)* — The desired total length of the resulting padded string.
- `ch` *(string | number, optional)* — The character to use for padding. Defaults to `' '` (space). Numbers are coerced to strings.

**Returns:** `string` — The input string left-padded with the specified character to at least the specified length.

**Behavior:**

- If `str` is already equal to or longer than `len`, the original string is returned unchanged (no truncation).
- If `ch` is omitted or `undefined`, a single space character is used.
- Numeric values for `str` or `ch` are coerced via standard string conversion.

## License

MIT
