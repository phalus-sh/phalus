{
  "documented_errors": "No explicit error types or thrown exceptions are documented in the public API for v1.1.3.",
  "implicit_behavior": {
    "invalid_len": "Passing non-numeric values for `len` may produce unexpected results but no specific error is documented.",
    "null_ch": "Behavior when `null` is passed for `ch` is not documented.",
    "null_str": "Behavior when `null` is passed for `str` is not documented; coercion behavior would depend on implementation."
  },
  "notes": "The package does not document any thrown Error objects, rejected Promises, or error codes. It is a synchronous, non-throwing utility by design intent."
}