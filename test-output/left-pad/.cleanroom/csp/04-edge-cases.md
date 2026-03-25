{
  "ch_omitted": "Omitting `ch` is equivalent to passing a space character ' '.",
  "ch_undefined": "Passing `undefined` for `ch` results in the default space character being used.",
  "empty_string": "If `str` is an empty string and `len` is positive, the result is a string consisting solely of `len` repetitions of the pad character.",
  "len_zero_or_negative": "If `len` is 0 or negative, no padding is added and the original coerced string is returned.",
  "multi_character_ch": "Behavior with multi-character strings for `ch` is not explicitly documented in the public API; `ch` is described as a character (singular), implying single-character usage is the intended pattern.",
  "numeric_ch": "Numeric values for `ch` are coerced via standard string conversion (e.g., the number 0 becomes '0').",
  "numeric_str": "Numeric values for `str` are coerced via standard string conversion (e.g., the number 5 becomes '5').",
  "str_length_equals_len": "If the length of `str` (after coercion) already equals `len`, the string is returned as-is with no padding added.",
  "str_length_exceeds_len": "If the length of `str` (after coercion) exceeds `len`, the original string is returned as-is. The function does NOT truncate."
}