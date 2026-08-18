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

### Size changes the answer

Both of this encoding's weaknesses arrive together, and they arrive with size. Mean
marginal tokens saved against each alternative, over 200 deterministic payloads per
cell, ranged across all five tokenizers — **positive means `unigram` is cheaper**:

| payload  | vs hex          | vs base64url    | `unigram` characters |
|----------|----------------:|----------------:|---------------------:|
| 4 bytes  | +1.1 … +3.9     | +0.6 … +2.3     |                   27 |
| 8 bytes  | +1.9 … +7.1     | +0.1 … +2.8     |                   55 |
| 16 bytes | +3.2 … +13.4    | −0.7 … +5.3     |                  111 |
| 32 bytes | +5.6 … +26.2    | −2.5 … +9.2     |                  224 |

At **4 bytes it beats hex, base64url, and base58 in every family measured**, with no
qualification. At 8 bytes it wins everywhere bar a dead tie with base58 under GPT-4o.
base64url only pulls ahead at 16 bytes and above, and only under the two newest GPT
vocabularies, which have memorised base64 fragments — under Claude it loses to
`unigram` by 5 to 9 tokens at exactly those sizes.

The character column moves the same way. Four words is 27 characters, which is nothing;
thirty-two words is 224, which is three wrapped lines and genuinely worse to read than
a 43-character base64 string. Store the bytes and the length costs you nothing at rest,
but an id that appears in a log line is being looked at, and that is the whole pitch.

So: this is unambiguously the right carrier at **nonce and correlation-id sizes**, which
is what it was built for, and it weakens steadily as payloads grow. For a 32-byte digest
that a human never reads, base64url under a GPT model is a defensible choice.

### What it is not

**Not error-detecting on its own.** All 256 symbols are occupied, so any word swapped
for another word decodes cleanly to different bytes. `CheckedUnigramId` adds a CRC-8
word for that; the plain type cannot and never will.

**Not a secret.** Readable is the point; unguessable is a separate property that comes
from width, and comparison here is not constant-time.

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

`parse` is **canonical**: lowercase alphabet words joined by exactly one space, and
nothing else. Exactly one accepted spelling per value, which is what belongs anywhere
the value is about to be trusted — a database key, an API parameter, an authorization
check.

`recover` is **tolerant**: any run of non-letters separates words, and case is
ignored. A value that came back hyphenated, re-wrapped, comma-joined, quoted, or
shouted still yields the bytes that were sent.

It reads *all* of what it is given, so it recovers a value that has been reformatted,
not one embedded in a sentence — every word present must be an alphabet word. Isolate
the candidate first. And under it `error message` is a valid encoded value, because
both are alphabet words, which is why it must not be the parser at a trust boundary.

### What the alphabet cannot do

All 256 symbols are occupied, so **every sequence of alphabet words is a valid
value**. A word swapped for another alphabet word, dropped, repeated, or transposed
decodes cleanly to different bytes, and nothing in the table can notice. No
arrangement of it changes that; it is what having no spare symbols means.

The alphabet's constraints — a character edit of at least two between entries, no
prefixes, no suffix-derivatives — are *risk reduction*, not detection. They make it
unlikely that a small slip lands on another valid word. They cannot make it noticeable
when one does.

`CheckedUnigramId` is the part that detects. One extra word carries a CRC-8 over the
payload, catching every single-word substitution and transposition outright, and an
arbitrary accidental mutation with probability about 255/256. It detects accidents,
not tampering: anyone who can change the payload can recompute the check word, so a
hostile party calls for a keyed MAC over the bytes.

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

### Against the alternatives

The full grid is [above](#size-changes-the-answer). What no
alternative has is the flat column: every `unigram` value of a given width costs
exactly the same number of tokens, so a budget is known before the value is minted,
where hex and base64 both have to be provisioned for their worst case.

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
