// MIT License
//
// Copyright (c) azer
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

'use strict';

var leftPad = require('../index');
var assert = require('assert');

var passed = 0;
var failed = 0;

function test(id, description, actual, expected) {
  try {
    assert.strictEqual(actual, expected);
    console.log('PASS [' + id + '] ' + description);
    passed++;
  } catch (e) {
    console.error('FAIL [' + id + '] ' + description);
    console.error('  Expected: ' + JSON.stringify(expected));
    console.error('  Actual:   ' + JSON.stringify(actual));
    failed++;
  }
}

// TC-01: Pad a short string with default space character
test('TC-01', 'Pad a short string with default space character',
  leftPad('foo', 5),
  '  foo'
);

// TC-02: Pad a string with a custom character
test('TC-02', 'Pad a string with a custom character',
  leftPad('foo', 5, '0'),
  '00foo'
);

// TC-03: String already equals target length
test('TC-03', 'String already equals target length',
  leftPad('hello', 5),
  'hello'
);

// TC-04: String exceeds target length
test('TC-04', 'String exceeds target length',
  leftPad('hello world', 5),
  'hello world'
);

// TC-05: Numeric str input
test('TC-05', 'Numeric str input',
  leftPad(42, 5, '0'),
  '00042'
);

// TC-06: Numeric ch input
test('TC-06', 'Numeric ch input',
  leftPad('7', 3, 0),
  '007'
);

// TC-07: Empty string padded
test('TC-07', 'Empty string padded',
  leftPad('', 3, '-'),
  '---'
);

// TC-08: len is 0
test('TC-08', 'len is 0',
  leftPad('hi', 0),
  'hi'
);

// TC-09: ch omitted, defaults to space
test('TC-09', 'ch omitted, defaults to space',
  leftPad('x', 4),
  '   x'
);

// TC-10: Pad with space character explicitly
test('TC-10', 'Pad with space character explicitly',
  leftPad('ab', 5, ' '),
  '   ab'
);

// TC-11: Single character string padded to length 1
test('TC-11', 'Single character string padded to length 1',
  leftPad('a', 1),
  'a'
);

// TC-12: Numeric str of 0
test('TC-12', 'Numeric str of 0',
  leftPad(0, 3, '0'),
  '000'
);

// Additional edge case tests

// ch undefined explicitly
test('EC-01', 'ch undefined explicitly uses space',
  leftPad('hi', 5, undefined),
  '   hi'
);

// Negative len
test('EC-02', 'Negative len returns original string',
  leftPad('hi', -1),
  'hi'
);

// Numeric str 5
test('EC-03', 'Numeric str 5 coerced to string',
  leftPad(5, 3, '0'),
  '005'
);

// Summary
console.log('\n' + passed + ' passed, ' + failed + ' failed');

if (failed > 0) {
  process.exit(1);
}
