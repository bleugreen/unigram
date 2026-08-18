# Changelog

## 0.2.0

**The wire format changed.** Values encoded by 0.1.x decode to different bytes under
0.2.0. There is no migration path and none is possible, because nothing in an encoded
value says which table produced it — which is itself why `FORMAT_VERSION` now exists.
This is the last moment such a change is cheap.

### The alphabet

- Four entries were reachable from another entry by dropping a suffix: `build`/
  `building`, `head`/`header`, `play`/`players`, `train`/`training`. Under those, a
  model normalising `training` to `train` produced a different byte and decoded
  silently. The edit-distance rule never covered this — `training` is four deletions
  from `train` — and a suffix change is far likelier from a model than a character
  slip. `building`, `header`, `players`, and `training` are replaced by `font`,
  `module`, `month`, and `park`, and a test now enforces the property.
- The table is pinned by a digest test, so changing an entry has to be deliberate
  rather than something a green suite lets through.
- `FORMAT_VERSION` records which table is in force.

### The API

- `UnigramId<const N: usize>` holds the bytes, renders the words through `Display`,
  and makes length part of the type. This is the recommended interface.
- `decode` is now **canonical**: lowercase words, single spaces, nothing else. The
  tolerant parser moved to `decode_recovered`. A tolerant parser is right for pulling
  a value out of model output and wrong at a trust boundary, and having only one of
  them meant the wrong one was the default.
- `matches` and `normalize` are **removed**. `matches` inferred a value's format from
  its syntax, so `matches("AbC", "abc")` and `matches("", "   ")` were both true, and
  a legacy string made of alphabet words was silently reinterpreted as unigram.
  Comparison belongs where the format is known, which is the caller.
- `mint` is replaced by `try_mint`, which reports an unavailable entropy source
  instead of panicking.
- `DecodeError::NotCanonical` distinguishes "would have decoded, spelled differently"
  from "not a word at all".

### Claims

Several were measured for the first time and found wrong.

- **The token claim is now scoped.** One token per byte is exact for every word a
  space precedes; the opening word costs one extra when it follows a backtick, an
  open paren, a quote, or a newline. The verifier now measures eight surrounding
  contexts in every family and fails if any exceeds one token of overhead.
- **"Densest form the trip allows"** contradicted this crate's own analysis showing
  509 entries would be denser. It is the densest *byte-aligned* form.
- **"Appended to, never rearranged"** described an array that is full at 256. It is
  frozen; there is nothing to append.
- **"Worst case"** was a sample maximum over 64 payloads, and now says so.
- The bijection is claimed over *nonempty* byte strings. `encode(&[])` is `""` and
  both parsers refuse it — deliberate for an identifier codec, and now tested.

### Verification

- Dependencies and the Llama tokenizer revision are pinned exactly.
- `rust-version = "1.78"` declared and built in CI.

## 0.1.0 – 0.1.5

Initial release, extracted from cairn. Superseded; the wire format differs.
