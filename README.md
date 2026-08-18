# unigram

A bijective codec between bytes and words that cost exactly one LLM token.

```text
a14ed61a                          ->  people error social career

8623a771b764ce50bb85371ff65aebe9  ->  login city population income question head
                                      season example region location count century
                                      update football task table
```

An identifier becomes something you can read. Say it out loud, carry it across a room
or between two windows, tell it apart from its neighbour at a glance, recognise it
again an hour later — the ordinary things a name affords. Ids spend their lives in
prompts, logs, and error messages, being looked at; this makes that free.

It is also the densest form the trip allows. One word is one byte and one token, so a
value costs exactly as many tokens as it carries bytes — flat, for every value, with
the spaces between words costing nothing at all. The four words above carry 32 bits
in 4 tokens; the sixteen carry 128 in 16.

## Using it

```rust
let words = unigram::encode(&[0x3d, 0x9a, 0x00, 0xff]);
assert_eq!(words, "department number access world");
assert_eq!(unigram::decode(&words)?, vec![0x3d, 0x9a, 0x00, 0xff]);

unigram::mint(4);                     // 32 fresh bits from the OS CSPRNG, 4 tokens
unigram::matches(issued, presented);  // comparison that forgives a round trip
```

`decode` is liberal in what it accepts: any run of characters that is not an ASCII
letter separates words, and case is ignored — so a value that came back hyphenated,
re-wrapped across lines, comma-joined, quoted, or shouted still decodes to the bytes
that were sent. It is exact in what it returns, though: an unknown word is refused
and named, never skipped or guessed at.

`matches` compares decoded bytes when both sides are encoded values, and normalized
strings otherwise — so values issued in some older format keep matching themselves
without a migration.

Store the bytes, not the words. Four bytes become twenty-six characters, which is a
poor thing to put in a column and index; `encode` is a table lookup and `decode` a
binary search per word, so the readable form is cheap to produce at whatever boundary
wants it — the prompt, the log line, the rendered page — and need not exist anywhere
else.

## What it costs

One word is one byte, and one word is one token, so an encoded value costs one token
per byte — the same for every value. Against hex of the same payload, under Claude:

| payload  | bits | hex (mean / worst) | `unigram` |
|----------|------|--------------------|-----------|
| 4 bytes  | 32   | 6.0 / 8            | 4         |
| 8 bytes  | 64   | 11.1 / 14          | 8         |
| 16 bytes | 128  | 21.5 / 25          | 16        |
| 32 bytes | 256  | 42.2 / 49          | 32        |

Roughly a quarter cheaper on average — but the flat cost matters more than the mean.
Hex swings with the value, so a token budget built on it has to assume the worst
case. This one is known before the value is minted.

The margin narrows under the GPT-4 vocabularies, where 32 bytes of hex average 37.1,
and widens sharply under Llama's SentencePiece, where the same payload averages 58.2
against the same flat 32.

## Choosing a length

Length is the entropy budget and the token budget at once — the two cannot drift
apart, which is most of why this is easier to size than hex.

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
or more, and at equal entropy the words are still the cheaper carrier.

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
quoted strings in practice, where embedded spaces are free — and `decode` accepts
every one of those separators anyway, so a value that comes back joined differently
is not a value that is lost.

## The alphabet

256 entries of lowercase ASCII English, 4 to 11 characters, chosen under four
constraints:

- **One token** under Claude, GPT-2/3 (`r50k`, `p50k`), GPT-3.5/4 (`cl100k`), GPT-4o
  (`o200k`), and Llama's SentencePiece — spanning both the BPE and SentencePiece
  families.
- **No two entries within one character edit of each other.** A slipped character
  lands outside the alphabet rather than on a different valid word, so `decode`
  refuses it instead of returning different bytes. This matters less than it sounds —
  nothing really mangles an id in practice — but it comes free with entries that have
  to be distinguishable to read in the first place.
- **Nothing charged** — no death, violence, race, gender, religion, or politics.
  These strings surface unbidden in transcripts, logs, and user-facing errors.
- **No entry is an inflection of another**, so a dropped plural cannot silently
  decode to a different byte.

The table is indexed by the byte each word encodes, so it is appended to, never
rearranged: reordering an entry changes what every previously issued value decodes to.

## Verifying it

The crate depends on nothing but the OS CSPRNG, at runtime or under test, and never
tokenizes. `cargo test` covers the codec and the table's structure; it says nothing
about cost.

Every number on this page is printed by `verify-alphabet.py`, which reads the
alphabet straight out of `src/lib.rs` and re-measures it against all five families:

```bash
uv run verify-alphabet.py
```

Run it after any edit to the table. A green test suite alone establishes none of what
this crate is named for.

## License

MIT.
