# Changelog

## 0.3.4

Documentation only.

Removed the sizing and collision analysis. How many bytes to ask for is the caller's
arithmetic, identical for any N-byte identifier, and a codec has no standing to run it —
`encode` takes the bytes it is given. The birthday table, the collision-versus-guessing
discussion, and the constant-time comparison note are all gone.

It also produced a wrong number nobody caught, which is what computing out-of-scope
things gets you: the two-word row read "fewer than 1", from a continuous approximation
applied below the range where it holds. One value cannot collide with anything.

Also fixes a real bug in `verify-claude.py`, which had never been run: it derived the
per-message frame from `count_tokens` on an empty string, which the API rejects. The
frame now comes from the carrier word instead.

## 0.3.3

Documentation only. Two rounds of external review pushed the docs into pre-emptive
self-defence, and each round added more of it. This removes it.

Gone: a section explaining that a byte-to-word codec is not an error-detecting code
(all 256 symbols are occupied, so every sequence is a valid value — that is how data
works, not a caveat); the hedging about whether a character slip is likely; and the
notes explaining that a parser which reads its whole input reads its whole input.
`CheckedUnigramId` is documented as the thing to reach for when a value must prove it
is intact, which is the useful half of what those sections were circling.

The cost comparison is also readable now. 0.3.2 led with a table of token *deltas* as
ranges across five tokenizers, in the second paragraph, before a reader knew what the
crate did — and its one scannable column was character count, the number that matters
least. It is now absolute token counts under Claude with worst-case in parentheses,
placed after the reader knows what the thing is, with the GPT exception in prose.

## 0.3.2

Documentation only.

0.3.1 caveated the token comparison with "base64url beats it, 29.9 vs 32.0" — which is
the single worst cell for `unigram`: the largest payload under the most base64-friendly
vocabulary. Cherry-picking against yourself distorts as much as the reverse, so the
comparison is now the full grid, and it says something more useful.

At **4 bytes `unigram` beats hex, base64url, and base58 in all five families**, without
qualification. At 8 bytes it wins everywhere except a tie with base58 under GPT-4o.
base64url pulls ahead only at 16 bytes and above and only under the two newest GPT
vocabularies; under Claude it loses by 5 to 9 tokens at those same sizes.

Both weaknesses — the token margin and the character count — arrive together, with
size. That makes the honest recommendation sharper than a blanket caveat: this is the
right carrier at nonce and correlation-id widths, and it weakens as payloads grow.

## 0.3.1

Documentation and tooling only; the wire format is unchanged from 0.3.0.

- **Prior art.** BIP39 is the obvious "why not just use an existing wordlist", and it
  now has a measured answer: only 349 of its 2048 words are single-token both ways
  across all five families, so a BIP39-derived encoding lands on the same 256 entries
  and the same 8 bits per token — without the bare-cost or context guarantees. Claude
  is the binding constraint there too, at 366.
- **Limitations are stated up front** rather than found in a subsection. base64url is
  cheaper on mean under GPT-4o and a fifth of the characters; the plain type cannot
  detect a word-for-word substitution; a readable id is not a secret.
- **`verify-claude.py`** cross-checks the Claude column against Anthropic's official
  `count_tokens` endpoint and reports where `ctok` disagrees. The unofficial
  reconstruction was the weakest source behind the crate's strongest claim.
- Links to the published crate from the README and the repository.

## 0.3.0

**The wire format changed again.** Values encoded by 0.2.x decode to different bytes.
`FORMAT_VERSION` is 3.

### The token claim was false, and is now true

0.2.0 documented "one token per byte, plus at most one for the opening word". A second
external review disproved it, and it reproduced: **22 of the 256 entries cost two or
three tokens when not preceded by a space**, so a value beginning with `council` cost
N+2 at the start of a string under three of the four GPT vocabularies.

The verifier missed it by construction. It measured one fixed payload per context, and
that payload's opening word — `account` — happened to be cheap. The one variable that
decides whether the bound holds was the one held fixed.

Both are fixed:

- The alphabet is rebuilt from a pool filtered on one-token cost **bare as well as
  space-prefixed**, in every family, and additionally on surviving every measured
  surrounding context. 492 of 573 candidates qualified; 256 were selected.
- The verifier now sweeps **all 256 entries** through the opening and closing
  positions of every context, rather than sampling one. That sweep immediately found a
  second defect the fixed payload had hidden: 33 entries cost up to +3 after a backtick
  under the Llama artifact. Those are gone from the table too.

The claim is now: one token per byte, plus at most one for punctuation immediately
before the value — with no list of exceptions.

### Mutation detection, honestly

The alphabet was described as if its edit-distance and suffix rules prevented silent
mutation. They cannot. All 256 symbols are occupied, so every sequence of alphabet
words is a valid value: a word swapped for another word, dropped, repeated, or
transposed decodes cleanly to different bytes. No arrangement of the table changes
that. The constraints are risk reduction, and the docs now say so.

- `CheckedUnigramId<N>` renders `N + 1` words, the last carrying a CRC-8 over the
  payload. It catches every single-word substitution and transposition outright —
  swept exhaustively in the tests — and an arbitrary accidental mutation with
  probability about 255/256. It detects accidents, not tampering.
- The alphabet also gained a no-prefix invariant. 0.2.0 had seven prefix pairs
  (`count`/`country`, `info`/`information`, `print`/`println`, and four more) where
  completing the shorter word yielded a different valid byte.

### Parser and API

- `NotCanonical` now means what it said: wrong case, and also leading, trailing,
  repeated, or non-space separators. Previously only a wrong-case word produced it and
  everything else was reported as an unknown word.
- Fixed-width parsing is bounded. `UnigramId::<4>::parse` on a million-word input no
  longer allocates proportionally to the input before reporting the length; it holds
  `N` bytes and counts the rest.
- An unrecognised word is truncated to 32 characters and control-escaped before it
  reaches a `DecodeError`. It is attacker-shaped text on its way to a log line.
- `decode_recovered` documents what it actually does: it recovers a *reformatted*
  value, not one embedded in prose. Every word present must be an alphabet word.

### Claims scoped

Tokenizer support now names exact artifacts rather than model families: OpenAI's four
BPE vocabularies, the `hf-internal-testing/llama-tokenizer` SentencePiece model at
revision `d02ad6cb`, and `ctok` 1.0.0 as an unofficial Claude reconstruction. Notably
this says nothing about Llama 3, which tokenizes with tiktoken.

The README compared only against hex, which flattered the result. It now also shows
base64url and base58, which beat `unigram` on mean under GPT-4o and lose badly under
Claude, alongside the character-length cost. What `unigram` uniquely has is flat cost
and readability, not the lowest mean.

## 0.2.0

Extracted the alphabet's silent-mutation hole found by a first review: four entries
were reachable from another by dropping a suffix (`build`/`building`,
`train`/`training`, and two more). Introduced `UnigramId<N>`, canonical versus tolerant
parsing, the frozen digest, `FORMAT_VERSION`, and `try_mint`; removed `matches` and
`normalize`, which inferred a value's format from its syntax.

## 0.1.0 – 0.1.5

Initial release, extracted from cairn. Superseded; the wire format differs.
