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

module.exports = leftPad;

/**
 * Left-pad a string with a padding character until it reaches the desired length.
 *
 * @param {string|number} str  - The value to pad. Numbers are coerced to strings.
 * @param {number}        len  - The desired total length of the resulting padded string.
 * @param {string|number} [ch] - The character to use for padding. Defaults to ' '.
 * @returns {string} The input string left-padded to at least the specified length.
 */
function leftPad(str, len, ch) {
  // Coerce str to string
  str = String(str);

  // If ch is not provided or is undefined, default to space
  if (ch === undefined || ch === null) {
    ch = ' ';
  } else {
    ch = String(ch);
  }

  // If the string is already at or beyond the desired length, return as-is
  if (str.length >= len) {
    return str;
  }

  // Calculate how many pad characters are needed
  var pad = len - str.length;

  // Build the padding string
  var padding = '';
  for (var i = 0; i < pad; i++) {
    padding += ch;
  }

  return padding + str;
}
