{
  "leftPad": {
    "coercion": "If `str` is a number, it is converted to its string representation before padding. If `ch` is a number, it is converted to its string representation before use as the pad character.",
    "default_pad_character": "When `ch` is not provided or is undefined, a single ASCII space character (U+0020) is used as the pad character.",
    "description": "Pads the left side of a string with a padding character until the string reaches the target length.",
    "no_padding_needed": "If `str` is already equal to or longer than `len`, the original string (coerced to string if necessary) is returned unchanged, without any truncation.",
    "padding_logic": "The function computes how many characters need to be prepended to `str` to reach length `len`. It prepends copies of `ch` (or the first character of `ch` if `ch` is a multi-character string is not explicitly documented, but `ch` is documented as a character) to the left of `str` until the total length equals `len`.",
    "return_value": "Always returns a string regardless of the type of `str`."
  }
}