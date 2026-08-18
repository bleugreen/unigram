# unigram

A bijective codec between bytes and words that cost exactly one LLM token.

```rust
let words = unigram::encode(&[0x3d, 0x9a, 0x00, 0xff]);
// "check music access world"

assert_eq!(unigram::decode(&words)?, vec![0x3d, 0x9a, 0x00, 0xff]);
```

## Why

Machine identifiers are routinely handed to a language model and asked back: an
acknowledgement token, a digest, a correlation id. Hexadecimal is the worst possible
carrier for that trip. It is expensive, because a hex run shreds into a fragment
every character or two under every tokenizer; and it is *undetectably* fragile,
because every character is drawn from the same sixteen, so a corrupted one still
looks like a valid digest.

`unigram` carries the same bytes as words drawn from a fixed alphabet of 256. Two
properties follow from that size, and they are the whole design.

**One word is exactly one byte.** Encoding is a table lookup per byte — no
bit-packing, no padding, no length convention. Every byte string has exactly one
encoding, and every sequence of alphabet words decodes.

**Every word is exactly one token.** An encoded value costs one token per byte, and
the same for every value. Against hex of the same payload, under Claude:

| payload  | hex (mean / worst) | `unigram` |
|----------|--------------------|-----------|
| 4 bytes  | 6.0 / 8            | 4         |
| 16 bytes | 21.5 / 25          | 16        |
| 32 bytes | 42.2 / 49          | 32        |

Roughly a quarter cheaper on average — but the flat cost matters more than the mean.
Hex swings with the value, so a token budget built on it has to assume the worst
case. This one is known before the value is minted.

**Corruption becomes visible.** The alphabet is 256 words out of every string that
could be written, and no two entries are within one character edit of each other, so
a mangled word is overwhelmingly likely to be no word at all. `decode` says so, and
names the word:

```rust
unigram::decode("check musix access")?;
// Err(UnknownWord { position: 1, word: "musix" })
```

Hex cannot do this. Every single-character corruption of a hex digest is another
valid hex digest.

## Surviving the round trip

`decode` is liberal in what it accepts. Any run of characters that is not an ASCII
letter separates words, and case is ignored — so a value that came back hyphenated,
re-wrapped across lines, comma-joined, quoted, or shouted still decodes to the bytes
that were sent.

```rust
unigram::mint(4);                        // 32 fresh bits, 4 tokens
unigram::matches(issued, presented);     // comparison that forgives the damage
```

`matches` compares decoded bytes when both sides are encoded values, and normalized
strings otherwise — so values issued in some older format keep matching themselves
without a migration.

## Choosing a length

One word is one byte and one token, so a value's length is its entropy budget and
its token budget at once — the two cannot drift apart, which is most of why this is
easier to reason about than hex.

| words | bits | distinct values | values before a 1-in-a-million collision |
|-------|------|-----------------|------------------------------------------|
| 2     | 16   | 65,536          | fewer than 1                             |
| 3     | 24   | 16.8 million    | 5                                        |
| 4     | 32   | 4.3 billion     | 92                                       |
| 6     | 48   | 281 trillion    | 23,700                                   |
| 8     | 64   | 1.8 × 10¹⁹      | 6 million                                |
| 16    | 128  | 3.4 × 10³⁸      | 2.6 × 10¹⁶                               |
| 32    | 256  | 1.2 × 10⁷⁷      | 4.8 × 10³⁵                               |

The right column is the birthday bound, `k ≈ √(2·N·p)`, and it is the column to size
against: collisions arrive at the square root of the space, not at the space. Sixteen
words is a UUID's width, thirty-two a SHA-256's.

Two questions hide in that table and it answers only one. **Collision** is the right
column — how many values may be outstanding before two coincide. **Guessing** is
separate: `mint` draws from the OS CSPRNG, so every bit is unpredictable, but four
words is 4.3 billion candidates, which is an afternoon for anything that can ask
freely. Four words suits a value that is scoped, short-lived, and rate-limited — an
acknowledgement nonce, a correlation id. A value a stranger can grind at wants eight
or more, and at equal entropy the words are still cheaper than the hex: 64 bits costs
8 tokens here against a mean of 11.2 and a worst case of 14.

## The join is a space, deliberately

Tokenizer vocabularies hold their canonical word entries space-prefixed, so the space
between two words is absorbed into the word that follows it and costs nothing. No
other separator is free. Measured across all five families, an eight-byte value:

| separator | GPT-4o | GPT-3.5/4 | GPT-3 | GPT-2 | Llama | Claude |
|-----------|-------:|----------:|------:|------:|------:|-------:|
| space     |      8 |         8 |     8 |     8 |     8 |      8 |
| `_` `.`   |      8 |         8 |    15 |    15 |    15 |     15 |
| `-`       |     11 |         9 |    15 |    15 |    15 |     15 |
| `,` `\n`  |  13–15 |     12–15 |    15 |    15 |    15 |     15 |

The join would cost almost as much as the payload. Encoded values travel inside
quoted strings in practice, where embedded spaces are free.

## The alphabet

256 entries of lowercase ASCII English, 4 to 11 characters, chosen under four
constraints:

- **One token** under Claude, GPT-2/3 (`r50k`, `p50k`), GPT-3.5/4 (`cl100k`), GPT-4o
  (`o200k`), and Llama's SentencePiece — spanning both the BPE and SentencePiece
  families.
- **No two entries within one character edit of each other**, which is what makes a
  single-character slip land outside the alphabet instead of on a different valid
  word.
- **Nothing charged** — no death, violence, race, gender, religion, or politics.
  These strings surface unbidden in transcripts, logs, and user-facing errors.
- **No entry is an inflection of another**, so a dropped plural cannot silently
  decode to a different byte.

The table is indexed by the byte each word encodes, so it is appended to, never
rearranged: reordering an entry changes what every previously issued value decodes
to.

## Verifying it

The crate depends on nothing but the OS CSPRNG, at runtime or under test, and never
tokenizes. `cargo test` covers the codec and the table's structure; it says nothing
about cost.

Every cost claim above is checked by `verify-alphabet.py`, which reads the alphabet
straight out of `src/lib.rs` and re-measures it against all five families:

```bash
uv run verify-alphabet.py
```

Run it after any edit to the table. A green test suite alone establishes none of what
this crate is named for.

## License

MIT.
