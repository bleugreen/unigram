# unigram

[![crates.io](https://img.shields.io/crates/v/unigram.svg)](https://crates.io/crates/unigram)
[![docs.rs](https://docs.rs/unigram/badge.svg)](https://docs.rs/unigram)
[![MIT](https://img.shields.io/crates/l/unigram.svg)](LICENSE)

A bijective codec between bytes and words that cost exactly one LLM token.

`cargo add unigram` · [crates.io](https://crates.io/crates/unigram) ·
[docs.rs](https://docs.rs/unigram) · [CHANGELOG](CHANGELOG.md)

```text
a14ed61a                          ->  password email share building

8623a771b764ce50bb85371ff65aebe9  ->  links change points high random found
                                      season events region light const case
                                      users field table support
```

An identifier becomes something you can read. Say it out loud, carry it across a room
or between two windows, tell it apart from its neighbour at a glance, recognise it
again an hour later — the ordinary things a name affords. Ids spend their lives in
prompts, logs, and error messages, being looked at; this makes that free.

One word is one byte and one token, so a value costs exactly as many tokens as it
carries bytes — flat, for every value, with the spaces between words costing nothing.
The four words above carry 32 bits in 4 tokens; the sixteen carry 128 in 16.

## Using it

```rust
use unigram::{UnigramId, CheckedUnigramId};

let id: UnigramId<4> = UnigramId::try_random()?;   // 32 fresh bits, 4 tokens
println!("{id}");                                  // "password email share building"

let returned = UnigramId::<4>::parse(&text)?;      // canonical: exact
let salvaged = UnigramId::<4>::recover(&text)?;    // tolerant: forgives a round trip

// One extra word of CRC-8, when a mutated value must not pass as a valid one.
let checked: CheckedUnigramId<4> = CheckedUnigramId::try_random()?;
```

The bytes are the value; the words are how it is displayed and parsed. Holding it that
way means the length is part of the type, equality is byte equality, and there is no
question of what format a given value is in — the question a string-shaped API cannot
answer and has to guess at.

Free functions (`encode`, `decode`, `decode_recovered`, `try_mint`) are there for
variable-length payloads.

### Two parsers

`parse` is canonical: lowercase alphabet words, single spaces, nothing else. One accepted
spelling per value, which is what belongs where a value is about to be trusted.

`recover` forgives what a round trip through a model does — case, separators, line
wrapping. It reads the whole input, so isolate the candidate first.

Both refuse an unknown word and name it.

## What it costs

One word is one byte and one token, so an N-byte value costs exactly N tokens, the same
for every value. Mean tokens under Claude, with the worst of 200 deterministic payloads
in parentheses:

| encoding  |     4 bytes |     8 bytes |    16 bytes |    32 bytes |
|-----------|------------:|------------:|------------:|------------:|
| `unigram` | **4.0 (4)** | **8.0 (8)** | **16 (16)** | **32 (32)** |
| hex       |     6.0 (9) |   11.3 (15) |   21.7 (27) |   42.6 (52) |
| base64url |     6.3 (9) |   10.8 (14) |   21.3 (25) |   41.2 (48) |
| base58    |     6.6 (9) |   10.9 (13) |   21.2 (26) |   42.0 (47) |

The parenthesised figure matters as much as the mean. Every other encoding's cost swings
with the value, so a budget built on one has to assume its worst case; this one is known
before the value is minted.

Hex loses everywhere, at every size, in every family. The GPT vocabularies have memorised
base64 fragments, which changes that ranking above 4 bytes — under `o200k`, base64url
averages 29.5 tokens for 32 bytes against a flat 32, while `unigram` still wins at 4
bytes (4.0 against 4.5). Nonce and correlation-id widths are what this was built for;
a 32-byte digest is a worse fit, at 224 characters and no token margin left under GPT.

### Every context

One token per byte holds space-prefixed **and bare**, so a value costs exactly N at
the start of a string, after a space, in JSON, and mid-sentence. The only surcharge is
punctuation immediately before it. Measured for a 4-byte value against an ideal of 4,
sweeping **all 256 entries** through the opening and closing positions, worst kept:

| context          | GPT-4o | GPT-3.5/4 | GPT-3 | GPT-2 | Llama | Claude |
|------------------|-------:|----------:|------:|------:|------:|-------:|
| start of string  |     +0 |        +0 |    +0 |    +0 |    +0 |     +0 |
| in prose, `X.`   |     +0 |        +0 |    +0 |    +0 |    +0 |     +0 |
| JSON `"id":"X"`  |     −1 |        +0 |    +0 |    +0 |    +0 |     +1 |
| after a newline  |     +0 |        +0 |    +0 |    +0 |    +0 |     +1 |
| after `id: `     |     −1 |        −1 |    −1 |    −1 |    −1 |     +0 |
| markdown `` `X` ``|    +1 |        +1 |    +1 |    +1 |    +1 |     +0 |
| after `(`        |     +1 |        +1 |    +1 |    +1 |    +1 |     +0 |

So: *one token per byte, plus at most one for punctuation immediately before it* — a
constant, never scaling with the payload, and negative where the context ends in a
space the value absorbs.

That is a property of the table, and it was not free. 0.2.0 shipped 22 entries costing
two or three tokens bare, so a value opening with `council` cost N+2 at the start of a
string — and its verifier tested one payload whose opening word happened to be cheap.
Both are fixed. The sweep is why the claim needs no exception list.

### Why not an existing wordlist?

BIP39, Diceware, the PGP word list, and `what3words` all predate this and all map
data to words. None was chosen for tokenizers, and it shows. BIP39 is the closest
comparison — 2048 words, which would be 11 bits each if they were all single tokens:

| wordlist  | words | single-token both ways, all families | usable alphabet |
|-----------|------:|-------------------------------------:|----------------:|
| BIP39     |  2048 |                              **349** | 256 → 8 bits/token |
| `unigram` |   256 |                                  256 | 256 → 8 bits/token |

Only 349 of BIP39's 2048 survive the filter, and Claude is the binding constraint at
366. Round 349 down to a power of two and a BIP39-derived encoding lands on exactly
256 entries and exactly 8 bits per token — the same density, from a list that also has
no bare-cost or surrounding-context guarantee.

BIP39 optimises for a different thing, and does it well: unique four-character
prefixes and human-transcription distance, for seed phrases read off paper. That is
worth having. It is not what makes a word cost one token.

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

## The join is a space

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

256 entries of lowercase ASCII English, 4 to 10 characters, under five constraints:

- **One token, space-prefixed and bare,** under every tokenizer the verifier pins:
  OpenAI's `r50k_base`, `p50k_base`, `cl100k_base`, `o200k_base`; the
  `hf-internal-testing/llama-tokenizer` SentencePiece artifact at revision `d02ad6cb`;
  and `ctok` 1.0.0's `"5.0"` counter, an **unofficial** offline reconstruction of
  Claude's tokenizer rather than Anthropic's own. Those exact artifacts are the claim
  — not every model that shares a name, and in particular not Llama 3, which
  tokenizes with tiktoken rather than the SentencePiece model checked here.
- **No two entries within one character edit, and none a prefix or suffix-derivative
  of another.** A slipped character, a dropped suffix, or a completed word lands
  outside the alphabet rather than on a different valid entry.
- **Nothing charged** — no death, violence, race, gender, religion, or politics. These
  strings surface unbidden in transcripts, logs, and user-facing errors.
- **No function words.** A value made of `that`, `which`, and `would` reads as damaged
  prose rather than as a name.
- **Frozen.** Byte `n` is `ALPHABET[n]`, all 256 slots are occupied, and changing an
  entry changes what every previously issued value decodes to. A test pins the table's
  digest. Nothing in an encoded value says which table produced it, so a system that
  stores these must record `FORMAT_VERSION` alongside them.

## Verifying it

The crate depends on nothing but the OS CSPRNG, at runtime or under test, and never
tokenizes. `cargo test` covers the codec and the table's structure — sorted, unique,
lengths, edit distance, prefix and suffix relationships, the frozen digest, and an
exhaustive sweep of every single-word substitution against the check word. It says
nothing about cost.

Every number on this page is printed by `verify-alphabet.py`, which reads the alphabet
straight out of `src/lib.rs`, re-measures every entry against all five families both
space-prefixed and bare, and sweeps all 256 entries through the opening and closing
positions of every context — with dependencies and the tokenizer revision pinned
exactly:

```bash
uv run verify-alphabet.py
```

Run it after any edit to the table. A green test suite alone establishes none of what
this crate is named for.

## License

MIT.
