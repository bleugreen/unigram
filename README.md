# unigram

A bijective codec between bytes and words that cost exactly one LLM token.

```text
a14ed61a                          ->  park events share category

8623a771b764ce50bb85371ff65aebe9  ->  love class play index query head search
                                      export references login country change
                                      update football target system
```

An identifier becomes something you can read. Say it out loud, carry it across a room
or between two windows, tell it apart from its neighbour at a glance, recognise it
again an hour later — the ordinary things a name affords. Ids spend their lives in
prompts, logs, and error messages, being looked at; this makes that free.

It is also the densest byte-aligned form the trip allows. One word is one byte and one
token, so a value costs exactly as many tokens as it carries bytes — flat, for every
value, with the spaces between words costing nothing at all. The four words above
carry 32 bits in 4 tokens; the sixteen carry 128 in 16.

## Using it

```rust
use unigram::UnigramId;

let id: UnigramId<4> = UnigramId::try_random()?;   // 32 fresh bits, 4 tokens
println!("{id}");                                  // "park events share category"

let returned = UnigramId::<4>::parse(&text)?;      // canonical: exact
let salvaged = UnigramId::<4>::recover(&text)?;    // tolerant: forgives a round trip
assert_eq!(id.as_bytes().len(), 4);
```

The bytes are the value; the words are how it is displayed and parsed. Holding it that
way means the length is part of the type, equality is byte equality, and there is no
question of what format a given value is in — the question a string-shaped API cannot
answer and has to guess at.

Free functions (`encode`, `decode`, `decode_recovered`, `try_mint`) are there for
variable-length payloads.

### Two parsers, deliberately

`parse` is **canonical**: lowercase alphabet words joined by exactly one space, and
nothing else. Exactly one accepted spelling per value, which is what belongs anywhere
the value is about to be trusted — a database key, an API parameter, an authorization
check.

`recover` is **tolerant**: any run of non-letters separates words, and case is
ignored. A value that came back hyphenated, re-wrapped, comma-joined, quoted, or
shouted still yields the bytes that were sent. That belongs where a value is being
pulled out of prose a model wrote, and nowhere else — under it, `home page` is a valid
encoded value, because both are alphabet words.

Both are exact in what they return: an unknown word is refused and named, never
skipped or guessed at.

## What it costs

One word is one byte, and one word is one token, so an encoded value costs one token
per byte. Against hex of the same payload, under Claude:

| payload  | bits | hex (mean / sample max) | `unigram` |
|----------|------|-------------------------|-----------|
| 4 bytes  | 32   | 6.0 / 8                 | 4         |
| 8 bytes  | 64   | 11.1 / 14               | 8         |
| 16 bytes | 128  | 21.5 / 25               | 16        |
| 32 bytes | 256  | 42.2 / 49               | 32        |

Roughly a quarter cheaper on average — but the flat cost matters more than the mean.
Hex swings with the value, so a token budget built on it has to assume the worst case.
This one is known before the value is minted. Those figures are over 64 deterministic
payloads per size, so the right-hand column is a sample maximum, not a proven bound.

The margin narrows under the GPT-4 vocabularies, where 32 bytes of hex average 37.1,
and widens sharply under Llama's SentencePiece, at 58.2 against the same flat 32.

### The exception worth knowing

One token per byte is exact for every word a space precedes — every word but the
first. The **opening** word costs one extra token when the character before it is not
a space. For a 4-byte value, against an ideal of 4:

| context          | GPT-4o | GPT-3.5/4 | GPT-2/3 | Llama | Claude |
|------------------|-------:|----------:|--------:|------:|-------:|
| start of string  |      4 |         4 |       4 |     4 |      4 |
| in prose, `X.`   |      4 |         4 |       4 |     4 |      4 |
| after `id: `     |      3 |         3 |       3 |     3 |      4 |
| after a newline  |      4 |         4 |       4 |     4 |      5 |
| JSON `"id":"X"`  |      3 |         4 |       4 |     4 |      5 |
| markdown `` `X` ``|     5 |         5 |       5 |     5 |      4 |
| after `(`        |      5 |         5 |       5 |     5 |      4 |

So the guarantee is *one token per byte, plus at most one for the opening word* — a
constant, not something that grows with the payload. Where the context already ends in
a space, the value absorbs it and comes in a token under.

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
separate: `try_random` draws from the OS CSPRNG, so every bit is unpredictable, but
four words is 4.3 billion candidates, which is an afternoon for anything that can ask
freely. Four words suits a value that is scoped, short-lived, and rate-limited — an
acknowledgement nonce, a correlation id. A value a stranger can grind at wants eight
or more.

Comparison is byte equality, which is not constant-time. A value used as a bearer
credential wants a constant-time comparison over `as_bytes`, which this crate
deliberately does not pretend to provide.

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

The join would cost almost as much as the payload. Encoded values travel inside quoted
strings in practice, where embedded spaces are free — and `recover` accepts every one
of those separators anyway, so a value that comes back joined differently is not lost.

## The alphabet

256 entries of lowercase ASCII English, 4 to 11 characters, chosen under five
constraints:

- **One token** under Claude, GPT-2/3 (`r50k`, `p50k`), GPT-3.5/4 (`cl100k`), GPT-4o
  (`o200k`), and Llama's SentencePiece — spanning both the BPE and SentencePiece
  families.
- **No two entries within one character edit of each other**, so a slipped character
  lands outside the alphabet rather than on a different valid word.
- **No entry reachable from another by adding or removing a suffix.** `build` and
  `building` may not both be entries. A model regurgitating text is far likelier to
  normalise a suffix than to mistype a character, which makes this the mutation worth
  ruling out — and 0.1.x shipped with four such pairs before a test enforced it.
- **Nothing charged** — no death, violence, race, gender, religion, or politics. These
  strings surface unbidden in transcripts, logs, and user-facing errors.
- **Frozen.** Byte `n` is `ALPHABET[n]`, all 256 slots are occupied, and changing an
  entry changes what every previously issued value decodes to. A test pins the table's
  digest. Nothing in an encoded value says which table produced it, so a system that
  stores these must record `FORMAT_VERSION` alongside them.

## Verifying it

The crate depends on nothing but the OS CSPRNG, at runtime or under test, and never
tokenizes. `cargo test` covers the codec and the table's structure — sorted, unique,
lengths, edit distance, suffix reachability, frozen digest. It says nothing about cost.

Every number on this page is printed by `verify-alphabet.py`, which reads the alphabet
straight out of `src/lib.rs` and re-measures it against all five families, with
dependencies and the tokenizer revision pinned exactly:

```bash
uv run verify-alphabet.py
```

Run it after any edit to the table. A green test suite alone establishes none of what
this crate is named for.

## License

MIT.
