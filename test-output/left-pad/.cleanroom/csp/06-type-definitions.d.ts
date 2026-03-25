{
  "module_format": "CommonJS module; the function is the module's default export (module.exports = leftPad).",
  "types": {
    "InputValue": "string | number — accepted types for the `str` argument",
    "Length": "number — the target total length of the padded result",
    "PadChar": "string | number | undefined — accepted types for the `ch` argument",
    "Result": "string — the padded string output"
  },
  "typescript_definition": "declare function leftPad(str: string | number, len: number, ch?: string | number): string; export = leftPad;"
}