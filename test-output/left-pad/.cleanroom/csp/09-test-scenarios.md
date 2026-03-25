{
  "scenarios": [
    {
      "description": "Pad a short string with default space character",
      "expected_output": "  foo",
      "id": "TC-01",
      "input": {
        "len": 5,
        "str": "foo"
      },
      "notes": "Two spaces prepended to reach length 5"
    },
    {
      "description": "Pad a string with a custom character",
      "expected_output": "00foo",
      "id": "TC-02",
      "input": {
        "ch": "0",
        "len": 5,
        "str": "foo"
      },
      "notes": "Two '0' characters prepended"
    },
    {
      "description": "String already equals target length",
      "expected_output": "hello",
      "id": "TC-03",
      "input": {
        "len": 5,
        "str": "hello"
      },
      "notes": "No padding added"
    },
    {
      "description": "String exceeds target length",
      "expected_output": "hello world",
      "id": "TC-04",
      "input": {
        "len": 5,
        "str": "hello world"
      },
      "notes": "No truncation; original string returned"
    },
    {
      "description": "Numeric str input",
      "expected_output": "00042",
      "id": "TC-05",
      "input": {
        "ch": "0",
        "len": 5,
        "str": 42
      },
      "notes": "Number coerced to '42', then padded"
    },
    {
      "description": "Numeric ch input",
      "expected_output": "007",
      "id": "TC-06",
      "input": {
        "ch": 0,
        "len": 3,
        "str": "7"
      },
      "notes": "ch=0 coerced to '0'"
    },
    {
      "description": "Empty string padded",
      "expected_output": "---",
      "id": "TC-07",
      "input": {
        "ch": "-",
        "len": 3,
        "str": ""
      },
      "notes": "Three dashes fill the entire length"
    },
    {
      "description": "len is 0",
      "expected_output": "hi",
      "id": "TC-08",
      "input": {
        "len": 0,
        "str": "hi"
      },
      "notes": "No padding, original string returned"
    },
    {
      "description": "ch omitted, defaults to space",
      "expected_output": "   x",
      "id": "TC-09",
      "input": {
        "len": 4,
        "str": "x"
      },
      "notes": "Three spaces prepended using default pad char"
    },
    {
      "description": "Pad with space character explicitly",
      "expected_output": "   ab",
      "id": "TC-10",
      "input": {
        "ch": " ",
        "len": 5,
        "str": "ab"
      },
      "notes": "Explicit space character behaves same as default"
    },
    {
      "description": "Single character string padded to length 1",
      "expected_output": "a",
      "id": "TC-11",
      "input": {
        "len": 1,
        "str": "a"
      },
      "notes": "Already at target length, no padding"
    },
    {
      "description": "Numeric str of 0",
      "expected_output": "000",
      "id": "TC-12",
      "input": {
        "ch": "0",
        "len": 3,
        "str": 0
      },
      "notes": "Number 0 coerced to '0', then padded to '000'"
    }
  ]
}