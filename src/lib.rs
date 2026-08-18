//! `unigram` — a bijective codec between bytes and words that cost one LLM token.
//!
//! Machine identifiers spend their lives being looked at: handed to a language model
//! and asked back, printed in a log, quoted in an error, read off a page by whoever
//! is debugging at the time. This crate carries them as words, so that an id becomes
//! something a reader can hold — `created office access world` can be said out
//! loud, told apart from its neighbour at a glance, and recognised again an hour
//! later, which is what a name is for.
//!
//! ```
//! use unigram::UnigramId;
//!
//! let id = UnigramId::from_bytes([0x3d, 0x9a, 0x00, 0xff]);
//! assert_eq!(id.to_string(), "created office access world");
//! assert_eq!(UnigramId::<4>::parse("created office access world").unwrap(), id);
//! ```
//!
//! The words come from a fixed alphabet of 256, and two properties follow from that
//! size — they are the whole design:
//!
//! - **One word is exactly one byte.** Encoding is a table lookup per byte with no
//!   bit-packing, no padding, and no length convention; decoding is its inverse.
//!   Every nonempty byte string has exactly one encoding, and every sequence of
//!   alphabet words decodes.
//! - **Every word is one token, wherever a space precedes it.** An encoded value
//!   therefore costs one token per byte. Against hex of the same payload, under
//!   Claude:
//!
//! | payload  | bits | hex (mean / sample max) | `unigram` |
//! |----------|------|-------------------------|-----------|
//! | 4 bytes  | 32   | 6.0 / 8                 | 4         |
//! | 8 bytes  | 64   | 11.1 / 14               | 8         |
//! | 16 bytes | 128  | 21.5 / 25               | 16        |
//! | 32 bytes | 256  | 42.2 / 49               | 32        |
//!
//!   Roughly a quarter cheaper on average, but the flat cost matters more than the
//!   mean: hex cost swings with the value, so a budget built on it has to assume the
//!   worst case. Those are Claude's numbers over 64 deterministic payloads per size,
//!   so the right-hand column is a sample maximum rather than a proven bound. The
//!   margin narrows under the GPT-4 vocabularies, where 32 bytes of hex average 37.1,
//!   and widens sharply under Llama's SentencePiece, at 58.2 against the same flat
//!   32. `verify-alphabet.py` prints the table for every family it checks.
//!
//! ## What the token claim covers
//!
//! Every entry costs one token **space-prefixed and bare**, in all five families, so
//! an N-byte value costs exactly N tokens at the start of a string, after a space,
//! inside a JSON string, and mid-sentence. The only surcharge is a punctuation
//! character immediately before the value — a backtick or an open parenthesis — which
//! adds one token, once, and is really the punctuation paying for itself. Measured
//! for a 4-byte value against an ideal of 4, sweeping **every one of the 256 entries**
//! through the opening and closing positions and keeping the worst:
//!
//! | context             | GPT-4o | GPT-3.5/4 | GPT-3 | GPT-2 | Llama | Claude |
//! |---------------------|-------:|----------:|------:|------:|------:|-------:|
//! | start of string     |     +0 |        +0 |    +0 |    +0 |    +0 |     +0 |
//! | in prose, `X.`      |     +0 |        +0 |    +0 |    +0 |    +0 |     +0 |
//! | JSON `"id":"X"`     |     -1 |        +0 |    +0 |    +0 |    +0 |     +1 |
//! | after a newline     |     +0 |        +0 |    +0 |    +0 |    +0 |     +1 |
//! | after `id: `        |     -1 |        -1 |    -1 |    -1 |    -1 |     +0 |
//! | markdown `` `X` ``  |     +1 |        +1 |    +1 |    +1 |    +1 |     +0 |
//! | after `(`           |     +1 |        +1 |    +1 |    +1 |    +1 |     +0 |
//!
//! So the guarantee is *one token per byte, plus at most one for punctuation
//! immediately before it* — a constant, never anything that scales with the payload,
//! and negative where the context ends in a space the value absorbs.
//!
//! This is a property of the table rather than a happy accident, and it was not free.
//! 0.2.0 shipped an alphabet in which 22 entries cost two or three tokens bare, so a
//! value beginning with `council` cost N+2 at the start of a string — and its
//! verifier tested one payload whose opening word happened to be cheap. Both the
//! table and the sweep are fixed. The sweep is why the claim above needs no list of
//! exceptions.
//!
//! ## Size
//!
//! This encoding's weakness is size. Mean marginal tokens saved against each
//! alternative, over 200 deterministic payloads per cell, ranged across all five
//! tokenizers — positive means `unigram` is cheaper:
//!
//! | payload  | vs hex       | vs base64url | vs base58    |
//! |----------|-------------:|-------------:|-------------:|
//! | 4 bytes  | +1.1 … +3.9  | +0.6 … +2.3  | +0.8 … +2.6  |
//! | 8 bytes  | +1.9 … +7.1  | +0.1 … +2.8  | −0.0 … +2.9  |
//! | 16 bytes | +3.2 … +13.4 | −0.7 … +5.3  | −0.6 … +5.2  |
//! | 32 bytes | +5.6 … +26.2 | −2.5 … +9.2  | −1.6 … +10.0 |
//!
//! Hex loses everywhere. At 4 bytes so does everything else, in every family. Above 16
//! bytes base64url costs a token or two less under the GPT vocabularies, which have
//! memorised base64 fragments; under Claude it costs five to nine more.
//!
//! It was built for nonce and correlation-id sizes, where it wins outright. A 32-byte
//! digest is a worse fit — the token margin is gone, and the value is 224 characters
//! across three wrapped lines rather than something read at a glance.
//!
//! Whatever the size, no alternative has the flat column: every value of a given width
//! costs the same, so a budget is known before minting, where hex and base64 must both
//! be provisioned for their worst case.
//!
//! ## The alphabet
//!
//! 256 entries of lowercase ASCII English, 4 to 10 characters, under five
//! constraints:
//!
//! - **One token, space-prefixed and bare,** under every tokenizer the verifier pins:
//!   OpenAI's `r50k_base`, `p50k_base`, `cl100k_base`, and `o200k_base`; the
//!   `hf-internal-testing/llama-tokenizer` SentencePiece artifact at revision
//!   `d02ad6cb`; and `ctok` 1.0.0's `"5.0"` counter, an *unofficial* offline
//!   reconstruction of Claude's tokenizer rather than Anthropic's own. Those exact
//!   artifacts are the claim — not every past or future model sharing a name, and in
//!   particular not Llama 3, which tokenizes with tiktoken rather than the
//!   SentencePiece model checked here.
//! - **No two entries within one character edit, and none a prefix or a
//!   suffix-derivative of another.** A slipped character, a dropped suffix, or a
//!   completed word lands outside the alphabet rather than on a different valid entry.
//!   For a value that must prove it is intact, see [`CheckedUnigramId`].
//! - **Nothing charged** — no death, violence, race, gender, religion, or politics.
//!   These strings surface unbidden in transcripts, logs, and user-facing errors.
//! - **No function words.** A value made of `that`, `which`, and `would` reads as
//!   damaged prose rather than as a name.
//! - **Frozen**, which is the next section.
//!
//! ## Why the join is a space
//!
//! Tokenizer vocabularies hold their canonical word entries space-prefixed, so the
//! space between two words is absorbed into the word that follows it and costs
//! nothing. No other separator is free. Measured across all five families, a hyphen,
//! comma, pipe, slash, or newline becomes a token of its own in every one of them,
//! taking an eight-byte value from 8 tokens to 15 — the join costing almost as much
//! as the payload. GPT-3.5/4 and GPT-4o absorb `_` and `.` for free; no other family
//! absorbs anything. Encoded values travel inside quoted strings in practice, where
//! embedded spaces are free.
//!
//! ## Reading a value back
//!
//! Two parsers, because they answer different questions.
//!
//! [`UnigramId::parse`] and [`decode`] are **canonical**: lowercase alphabet words
//! joined by exactly one space, nothing else. That is what belongs at a boundary
//! where the value is about to be trusted — a database key, an API parameter, an
//! authorization check — because a canonical parser has exactly one accepted spelling
//! per value, and cannot be talked into treating some other string as one.
//!
//! [`UnigramId::recover`] and [`decode_recovered`] are **tolerant**: any run of
//! characters that is not an ASCII letter separates words, and case is ignored, so a
//! value that came back hyphenated, re-wrapped, comma-joined, quoted, or shouted still
//! yields the bytes that were sent. It reads the whole input, so isolate the candidate
//! first.
//!
//! Both refuse an unknown word and name it.
//!
//! ## Why the alphabet is 256 and not larger
//!
//! A wider alphabet would carry more bits per token, so it is worth saying why this
//! one stops where it does. Of the roughly 65,000 space-prefixed lowercase words in
//! the largest vocabulary, 6,654 are single-token in all five families; 5,452 of
//! those are 4 to 11 ASCII characters; and 640 of *those* survive Claude, whose
//! tokenizer is by far the narrowest of the five. Spacing them a character edit apart
//! leaves about 509.
//!
//! So the ceiling is 512 entries — `log2(509) ≈ 8.99` bits per token against the 8
//! here, and 9 does not divide 8. Bit-packing 9-bit symbols would save nothing at all
//! on a 4-byte value (32 bits still needs 4 words), one token on a 16-byte value, and
//! three on a 32-byte one, in exchange for the byte-indexed table, the claim that one
//! word is one byte, and a codec that can be described in a sentence. It is not a
//! trade worth making, and this is therefore not the densest possible encoding — it
//! is the densest byte-aligned one.
//!
//! Other scripts do not change this. CJK is denser on the page but agrees across
//! families far less: 39 characters are single-token in all five, which does not
//! reach even 256. Accented Latin is worse — 5 words survive. The binding constraint
//! was never English; it is the intersection itself.
//!
//! Nor does an existing wordlist. BIP39 holds 2048 words, which would be 11 bits
//! each, but it was chosen for human transcription rather than for tokenizers: only
//! 349 of them are single-token both ways across all five families, Claude again
//! being the narrowest at 366. Rounded down to a power of two that is 256 entries and
//! 8 bits per token — the same density this reaches, from a list that carries no
//! bare-cost or surrounding-context guarantee.
//!
//! ## The alphabet is the wire format
//!
//! [`ALPHABET`] is frozen. Byte `n` is `ALPHABET[n]`, all 256 slots are occupied, and
//! changing any entry changes what every previously issued value decodes to. There is
//! no append: the array is full. A test pins the table's digest so that an edit has
//! to be deliberate, and if a different table is ever wanted it belongs beside this
//! one under a new name and a new [`FORMAT_VERSION`], with this decoder kept forever.
//!
//! Nothing in an encoded value says which table produced it, so a system that stores
//! these must record the format version alongside them, or accept that it can never
//! change tables.
//!
//! ## Changing the alphabet
//!
//! Nothing here tokenizes, at runtime or under test: the OS CSPRNG is this crate's
//! only dependency at any stage. So `cargo test` covers the codec's behaviour and the
//! table's structural properties — 256 entries, sorted, unique, 4 to 11 lowercase
//! ASCII characters, no two within one character edit, no entry reachable from
//! another by adding or removing a suffix, and the frozen digest — and says nothing
//! about cost.
//!
//! Every cost claim above is checked instead by `verify-alphabet.py`, beside this
//! file. It reads [`ALPHABET`] straight out of this source — a copy would drift — and
//! re-measures each entry against all five tokenizer families, along with the
//! composed per-byte cost in each surrounding context, the margin over hex, and the
//! choice of separator:
//!
//! ```text
//! uv run verify-alphabet.py
//! ```
//!
//! Run it after any edit to [`ALPHABET`]. A green test suite alone establishes none
//! of what this crate is named for, and an edit that satisfies every test here can
//! still break every cost claim above.

#![forbid(unsafe_code)]

use std::fmt;

/// The version of the encoding this crate implements.
///
/// Bumped only when [`ALPHABET`] changes, which changes what every previously issued
/// value decodes to. Nothing in an encoded value carries this, so a system storing
/// values must record it alongside them.
pub const FORMAT_VERSION: u32 = 3;

/// The 256-word alphabet, sorted, indexed by the byte each word encodes.
///
/// Sorted so the parsers can binary-search it, and byte `n` is `ALPHABET[n]` — the
/// table *is* the codec.
///
/// **Frozen.** All 256 slots are occupied, so there is nothing to append to, and
/// changing an entry changes what every previously issued value decodes to. A test
/// pins the digest of this table; if it fails, the change was not intended, and if it
/// was, it needs a new table under a new name rather than an edit to this one.
///
/// Laid out packed rather than one entry per line: rustfmt would give this table 256
/// vertical lines, which is harder to scan and to review than a grid, and the entries
/// are data rather than code.
///
/// Editing this table? A green `cargo test` proves only its structure. Run
/// `verify-alphabet.py` — that is where single-token cost is checked, under all five
/// tokenizer families.
#[rustfmt::skip]
pub const ALPHABET: [&str; 256] = [
    "access", "account", "action", "active", "added", "address", "album", "align", "android",
    "append", "area", "args", "array", "article", "author", "available", "background", "band",
    "based", "black", "board", "body", "books", "border", "born", "break", "building", "built",
    "button", "called", "card", "case", "category", "center", "central", "change", "character",
    "check", "children", "city", "class", "click", "client", "close", "club", "code", "color",
    "column", "command", "common", "community", "company", "component", "config", "console",
    "const", "container", "content", "control", "country", "course", "created", "current",
    "database", "date", "days", "default", "define", "design", "details", "device", "display",
    "document", "door", "double", "download", "east", "element", "email", "error", "events",
    "example", "export", "express", "external", "face", "false", "family", "father", "features",
    "field", "files", "film", "final", "find", "float", "font", "football", "force", "format",
    "found", "free", "function", "game", "general", "github", "global", "google", "green", "group",
    "header", "height", "help", "high", "history", "home", "house", "https", "human", "images",
    "import", "include", "index", "input", "install", "items", "json", "label", "language", "large",
    "length", "level", "library", "light", "links", "local", "location", "login", "market",
    "master", "match", "material", "media", "members", "message", "method", "models", "module",
    "month", "music", "named", "network", "number", "object", "office", "online", "options",
    "original", "output", "package", "params", "password", "people", "period", "person", "place",
    "player", "points", "position", "power", "press", "price", "println", "private", "process",
    "product", "program", "project", "property", "public", "python", "query", "question", "random",
    "range", "react", "record", "region", "register", "related", "release", "render", "report",
    "request", "require", "response", "results", "return", "review", "river", "route", "running",
    "school", "score", "script", "search", "season", "section", "security", "select", "series",
    "server", "service", "session", "share", "social", "software", "source", "space", "special",
    "species", "split", "square", "start", "states", "static", "station", "story", "street",
    "string", "student", "style", "success", "support", "system", "table", "target", "template",
    "title", "token", "track", "training", "types", "union", "update", "username", "users",
    "values", "version", "video", "views", "water", "width", "window", "words", "world"
];

/// An identifier of `N` bytes, rendered as `N` alphabet words.
///
/// The bytes are the value; the words are how it is displayed and parsed. Holding it
/// this way means the length is part of the type, equality is byte equality rather
/// than string comparison, and there is no question of what format a given value is
/// in — which is the question a string-shaped API cannot answer and must guess at.
///
/// ```
/// use unigram::UnigramId;
///
/// let id: UnigramId<4> = UnigramId::try_random().unwrap();
/// let round_tripped = UnigramId::<4>::parse(&id.to_string()).unwrap();
/// assert_eq!(id, round_tripped);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnigramId<const N: usize>([u8; N]);

impl<const N: usize> UnigramId<N> {
    /// Wrap bytes that are already in hand.
    pub const fn from_bytes(bytes: [u8; N]) -> Self {
        Self(bytes)
    }

    /// Mint `N` bytes of fresh entropy from the OS CSPRNG.
    pub fn try_random() -> Result<Self, getrandom::Error> {
        let mut bytes = [0u8; N];
        getrandom::fill(&mut bytes)?;
        Ok(Self(bytes))
    }

    /// Parse the **canonical** form: lowercase alphabet words, single spaces, nothing
    /// else.
    ///
    /// This is the parser for a boundary where the value is about to be trusted. For
    /// a value being retrieved out of text a model wrote, see [`UnigramId::recover`].
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        if text.is_empty() {
            return Err(DecodeError::Empty.into());
        }
        collect_exact(canonical_bytes(text)).map(Self)
    }

    /// Parse **tolerantly**, forgiving the reformatting a round trip through a model
    /// or a transcript introduces.
    ///
    /// Any run of characters that is not an ASCII letter separates words, and case is
    /// ignored. Reads the whole input, so isolate the candidate first, and use
    /// [`UnigramId::parse`] at a trust boundary.
    pub fn recover(text: &str) -> Result<Self, ParseError> {
        collect_exact(recovered_bytes(text)).map(Self)
    }

    /// The bytes this identifier carries.
    ///
    /// Store these. The word form is a rendering, cheap to produce wherever it will
    /// actually be read, and `N` bytes is a far better thing to keep in a column than
    /// the string.
    pub const fn as_bytes(&self) -> &[u8; N] {
        &self.0
    }

    /// Consume the identifier, yielding its bytes.
    pub const fn into_bytes(self) -> [u8; N] {
        self.0
    }
}

impl<const N: usize> fmt::Display for UnigramId<N> {
    /// The canonical rendering: `N` alphabet words joined by single spaces.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, byte) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str(" ")?;
            }
            f.write_str(ALPHABET[*byte as usize])?;
        }
        Ok(())
    }
}

impl<const N: usize> From<[u8; N]> for UnigramId<N> {
    fn from(bytes: [u8; N]) -> Self {
        Self(bytes)
    }
}

impl<const N: usize> From<UnigramId<N>> for [u8; N] {
    fn from(id: UnigramId<N>) -> Self {
        id.0
    }
}

/// CRC-8, the polynomial from SMBus/ATM (`x^8 + x^2 + x + 1`).
///
/// Position-dependent by construction, which is what makes it catch the mutations
/// an alphabet cannot: a word swapped for another valid word, two words
/// transposed, a word dropped or repeated. A plain XOR or sum would miss every
/// reordering.
const fn crc8(bytes: &[u8]) -> u8 {
    let mut crc: u8 = 0;
    let mut index = 0;
    while index < bytes.len() {
        crc ^= bytes[index];
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 0x80 != 0 {
                (crc << 1) ^ 0x07
            } else {
                crc << 1
            };
            bit += 1;
        }
        index += 1;
    }
    crc
}

/// An identifier of `N` bytes carrying a trailing check word, rendered as `N + 1`
/// alphabet words.
///
/// One extra word is 8 bits of CRC over the payload, so every single-word substitution
/// and transposition is caught outright, and an arbitrary accidental mutation with
/// probability about 255/256. Use it where a value has to prove it is the one that was
/// issued without the original in hand.
///
/// It detects accidents, not tampering: anyone who can change the payload can recompute
/// the check word. A hostile party calls for a keyed MAC over
/// [`CheckedUnigramId::as_bytes`], which this crate does not provide.
///
/// ```
/// use unigram::{CheckedUnigramId, ParseError};
///
/// let id = CheckedUnigramId::from_bytes([1u8, 2, 3, 4]);
/// let text = id.to_string();
/// assert_eq!(text.split(' ').count(), 5);          // four payload words, one check
/// assert_eq!(CheckedUnigramId::<4>::parse(&text).unwrap(), id);
///
/// // Swap one word for another valid word: caught, where UnigramId could not.
/// let mangled = text.replacen("account", "action", 1);
/// assert!(matches!(
///     CheckedUnigramId::<4>::parse(&mangled),
///     Err(ParseError::ChecksumMismatch { .. })
/// ));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedUnigramId<const N: usize>([u8; N]);

impl<const N: usize> CheckedUnigramId<N> {
    /// Wrap bytes that are already in hand. The check word is derived, never stored.
    pub const fn from_bytes(bytes: [u8; N]) -> Self {
        Self(bytes)
    }

    /// Mint `N` bytes of fresh entropy from the OS CSPRNG.
    pub fn try_random() -> Result<Self, getrandom::Error> {
        UnigramId::<N>::try_random().map(|id| Self(id.into_bytes()))
    }

    /// Parse the canonical form and verify the check word.
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        if text.is_empty() {
            return Err(DecodeError::Empty.into());
        }
        Self::verify(collect_checked(canonical_bytes(text))?)
    }

    /// Parse tolerantly and verify the check word.
    ///
    /// The pairing worth noting: tolerant parsing is what lets a value survive a
    /// round trip, and the check word is what keeps that tolerance from quietly
    /// accepting a value the trip changed.
    pub fn recover(text: &str) -> Result<Self, ParseError> {
        Self::verify(collect_checked(recovered_bytes(text))?)
    }

    /// The bytes this identifier carries, without the check word.
    pub const fn as_bytes(&self) -> &[u8; N] {
        &self.0
    }

    /// Consume the identifier, yielding its bytes.
    pub const fn into_bytes(self) -> [u8; N] {
        self.0
    }

    /// The check byte this payload computes.
    pub const fn check_byte(&self) -> u8 {
        crc8(&self.0)
    }

    fn verify((payload, found): ([u8; N], u8)) -> Result<Self, ParseError> {
        let expected = crc8(&payload);
        if expected != found {
            return Err(ParseError::ChecksumMismatch { expected, found });
        }
        Ok(Self(payload))
    }
}

impl<const N: usize> fmt::Display for CheckedUnigramId<N> {
    /// `N` payload words, then the check word, joined by single spaces.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.0 {
            f.write_str(ALPHABET[*byte as usize])?;
            f.write_str(" ")?;
        }
        f.write_str(ALPHABET[self.check_byte() as usize])
    }
}

/// Why a sequence of words could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// A word outside the alphabet, and where in the sequence it sat.
    ///
    /// Reported rather than skipped or guessed at: a value that lost a word is not
    /// the value that was sent, and inventing the byte it stood for would answer a
    /// question nobody asked with a value nobody issued.
    UnknownWord { position: usize, word: String },
    /// Well-formed alphabet words, but not in canonical spelling — wrong case, or
    /// separated by something other than a single space. Only [`decode`] reports
    /// this; [`decode_recovered`] accepts these and returns the bytes.
    NotCanonical { position: usize },
    /// No alphabet words at all. The encoding of no bytes is the empty string, which
    /// is never a value a caller means to transmit, so decoding one is an error
    /// rather than an empty success. [`encode`] still renders `&[]` as `""`; the
    /// asymmetry is deliberate, and it is why the bijection is claimed over nonempty
    /// byte strings.
    Empty,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownWord { position, word } => write!(
                f,
                "`{word}` (word {}) is not in the unigram alphabet",
                position + 1
            ),
            Self::NotCanonical { position } => write!(
                f,
                "word {} is not in canonical form (lowercase, single-space separated)",
                position + 1
            ),
            Self::Empty => f.write_str("no unigram words found"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Why a string could not be parsed as a [`UnigramId`] of a particular width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The words themselves did not decode.
    Decode(DecodeError),
    /// The words decoded, but there were the wrong number of them.
    WrongLength { expected: usize, found: usize },
    /// Every word was an alphabet entry and the count was right, but the trailing
    /// check word does not match the payload. See [`CheckedUnigramId`].
    ChecksumMismatch { expected: u8, found: u8 },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(error) => error.fmt(f),
            Self::WrongLength { expected, found } => {
                write!(f, "expected {expected} words, found {found}")
            }
            Self::ChecksumMismatch { expected, found } => write!(
                f,
                "check word is `{}`, but the payload computes `{}`",
                ALPHABET[*found as usize], ALPHABET[*expected as usize]
            ),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            Self::WrongLength { .. } | Self::ChecksumMismatch { .. } => None,
        }
    }
}

impl From<DecodeError> for ParseError {
    fn from(error: DecodeError) -> Self {
        Self::Decode(error)
    }
}

/// Encode bytes as space-joined alphabet words, one word per byte.
///
/// Cheap enough to call at a boundary rather than storing the result: four bytes
/// become twenty-six characters, which is a poor thing to keep in a column, and this
/// is a table lookup per byte in each direction. Store the bytes; render the words
/// wherever they will actually be read.
///
/// Encoding no bytes yields the empty string, which [`decode`] refuses. See
/// [`DecodeError::Empty`].
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 8);
    for (index, byte) in bytes.iter().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        out.push_str(ALPHABET[*byte as usize]);
    }
    out
}

/// At most this much of an offending word is repeated back in a [`DecodeError`].
///
/// The word comes from whatever arrived, so it is attacker-shaped: unbounded, and
/// free to contain newlines or control characters that would rearrange a log line.
/// Errors are for reading, so it is truncated and escaped.
const WORD_PREVIEW: usize = 32;

fn preview(word: &str) -> String {
    let mut out = String::with_capacity(WORD_PREVIEW);
    for character in word.chars().take(WORD_PREVIEW) {
        match character {
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    if word.chars().nth(WORD_PREVIEW).is_some() {
        out.push('…');
    }
    out
}

/// Why a word failed canonical parsing.
///
/// Distinguishing these is the point: a value spelled differently is a caller to
/// correct, and a word that is nothing at all is a value to reject.
fn classify(word: &str, position: usize) -> DecodeError {
    // An empty segment means the spacing was wrong -- leading, trailing, or
    // repeated spaces -- rather than that some word was unrecognisable.
    if word.is_empty() {
        return DecodeError::NotCanonical { position };
    }
    if word.is_ascii()
        && ALPHABET
            .binary_search(&word.to_ascii_lowercase().as_str())
            .is_ok()
    {
        return DecodeError::NotCanonical { position };
    }
    // Every piece of it is an alphabet word, so what is wrong is the separator
    // holding them together, not the words.
    let mut pieces = recovered_words(word).peekable();
    if pieces.peek().is_some()
        && pieces.all(|piece| {
            ALPHABET
                .binary_search(&piece.to_ascii_lowercase().as_str())
                .is_ok()
        })
    {
        return DecodeError::NotCanonical { position };
    }
    DecodeError::UnknownWord {
        position,
        word: preview(word),
    }
}

fn recovered_words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_ascii_alphabetic())
        .filter(|word| !word.is_empty())
}

/// The bytes of a canonical rendering, one at a time.
fn canonical_bytes(text: &str) -> impl Iterator<Item = Result<u8, DecodeError>> + '_ {
    text.split(' ')
        .enumerate()
        .map(|(position, word)| match ALPHABET.binary_search(&word) {
            Ok(index) => Ok(index as u8),
            Err(_) => Err(classify(word, position)),
        })
}

/// The bytes of a tolerantly-read rendering, one at a time.
fn recovered_bytes(text: &str) -> impl Iterator<Item = Result<u8, DecodeError>> + '_ {
    recovered_words(text).enumerate().map(|(position, word)| {
        let lowered = word.to_ascii_lowercase();
        match ALPHABET.binary_search(&lowered.as_str()) {
            Ok(index) => Ok(index as u8),
            Err(_) => Err(DecodeError::UnknownWord {
                position,
                word: preview(word),
            }),
        }
    })
}

/// Read exactly `N` bytes from a stream of them.
///
/// Never holds more than `N` bytes, whatever arrives: a value that is too long is
/// counted to the end but stored only up to `N`, so an enormous input costs time
/// rather than memory. That is most of the point of knowing the width up front.
fn collect_exact<const N: usize>(
    stream: impl Iterator<Item = Result<u8, DecodeError>>,
) -> Result<[u8; N], ParseError> {
    let mut out = [0u8; N];
    let mut found = 0usize;
    for byte in stream {
        let byte = byte?;
        if let Some(slot) = out.get_mut(found) {
            *slot = byte;
        }
        found += 1;
    }
    if found != N {
        return Err(ParseError::WrongLength { expected: N, found });
    }
    Ok(out)
}

/// Read `N` payload bytes and one trailing check byte, holding no more than that.
///
/// Split out from [`collect_exact`] rather than asking for `N + 1`, which stable
/// Rust cannot express as an array width.
fn collect_checked<const N: usize>(
    stream: impl Iterator<Item = Result<u8, DecodeError>>,
) -> Result<([u8; N], u8), ParseError> {
    let mut payload = [0u8; N];
    let mut check = 0u8;
    let mut found = 0usize;
    for byte in stream {
        let byte = byte?;
        if let Some(slot) = payload.get_mut(found) {
            *slot = byte;
        } else if found == N {
            check = byte;
        }
        found += 1;
    }
    if found != N + 1 {
        return Err(ParseError::WrongLength {
            expected: N + 1,
            found,
        });
    }
    Ok((payload, check))
}

/// Decode the **canonical** form: lowercase alphabet words joined by single spaces.
///
/// Exactly one accepted spelling per value, which is what makes this the parser to
/// use where a value is about to be trusted. For text that has been through a model,
/// see [`decode_recovered`].
pub fn decode(text: &str) -> Result<Vec<u8>, DecodeError> {
    if text.is_empty() {
        return Err(DecodeError::Empty);
    }
    canonical_bytes(text).collect()
}

/// Decode **tolerantly**, forgiving the reformatting a round trip introduces.
///
/// Any run of characters that is not an ASCII letter separates words, and case is
/// ignored, so a value that came back hyphenated, re-wrapped across lines,
/// comma-joined, quoted, or shouted still yields the bytes that were sent.
///
/// Reads the whole input, so isolate the candidate first. Use [`decode`] at a trust
/// boundary, and [`CheckedUnigramId`] where a value has to prove it is one.
pub fn decode_recovered(text: &str) -> Result<Vec<u8>, DecodeError> {
    let bytes: Vec<u8> = recovered_bytes(text).collect::<Result<_, _>>()?;
    if bytes.is_empty() {
        return Err(DecodeError::Empty);
    }
    Ok(bytes)
}

/// Mint `bytes` bytes of fresh entropy, encoded.
///
/// See [`UnigramId::try_random`] for the typed form.
pub fn try_mint(bytes: usize) -> Result<String, getrandom::Error> {
    let mut buffer = vec![0u8; bytes];
    getrandom::fill(&mut buffer)?;
    Ok(encode(&buffer))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything cost-related lives in `verify-alphabet.py`, which measures the five
    /// tokenizer families this crate claims. What is left here is what can be checked
    /// without a vocabulary: the table's structure, and the codec over it.
    #[test]
    fn the_alphabet_is_sorted_unique_and_plain_lowercase() {
        let mut sorted = ALPHABET;
        sorted.sort_unstable();
        assert_eq!(sorted, ALPHABET, "binary_search requires sorted order");
        let unique: std::collections::HashSet<_> = ALPHABET.iter().collect();
        assert_eq!(unique.len(), ALPHABET.len());
        for word in ALPHABET {
            assert!(
                word.len() >= 4 && word.len() <= 11 && word.bytes().all(|b| b.is_ascii_lowercase()),
                "`{word}`"
            );
        }
    }

    /// The table is the wire format. This pins it, so that changing an entry has to
    /// be a deliberate act with this constant updated alongside it, rather than an
    /// edit that leaves every previously issued value decoding to something else
    /// while the suite stays green.
    #[test]
    fn the_alphabet_matches_its_frozen_digest() {
        const FROZEN: u64 = 0x3e7c_f24c_a1a4_56f6;
        const PRIME: u64 = 0x0000_0100_0000_01b3;
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for (index, word) in ALPHABET.iter().enumerate() {
            if index > 0 {
                hash = (hash ^ u64::from(b'\n')).wrapping_mul(PRIME);
            }
            for byte in word.bytes() {
                hash = (hash ^ u64::from(byte)).wrapping_mul(PRIME);
            }
        }
        assert_eq!(
            hash, FROZEN,
            "the alphabet changed: every previously issued value now decodes differently"
        );
    }

    /// Distance from every other entry is what turns a one-character slip into a
    /// refusal instead of a different valid byte.
    #[test]
    fn no_two_entries_are_within_one_edit_of_each_other() {
        for (i, a) in ALPHABET.iter().enumerate() {
            for b in &ALPHABET[i + 1..] {
                assert!(!within_one_edit(a, b), "`{a}` and `{b}` are one edit apart");
            }
        }
    }

    fn within_one_edit(a: &str, b: &str) -> bool {
        let (a, b) = if a.len() > b.len() { (b, a) } else { (a, b) };
        let (short, long) = (a.as_bytes(), b.as_bytes());
        match long.len() - short.len() {
            0 => short.iter().zip(long).filter(|(x, y)| x != y).count() <= 1,
            1 => {
                let skip = short.iter().zip(long).take_while(|(x, y)| x == y).count();
                short[skip..] == long[skip + 1..]
            }
            _ => false,
        }
    }

    /// The edit-distance rule does not reach morphology: `training` is four deletions
    /// from `train`, so nothing above stops both from being entries — and 0.1.x
    /// shipped with four such pairs. A model regurgitating text is far likelier to
    /// normalise a suffix than to mistype a character, which makes this the mutation
    /// worth ruling out, and it is only ruled out if it is tested.
    #[test]
    fn no_entry_is_reachable_from_another_by_a_suffix() {
        const SUFFIXES: [&str; 15] = [
            "s", "es", "ing", "ed", "er", "ers", "ors", "ion", "ions", "ies", "ment", "ments",
            "al", "ly", "y",
        ];
        for entry in ALPHABET {
            for suffix in SUFFIXES {
                for stem in [
                    entry.to_string(),
                    entry.strip_suffix('e').unwrap_or(entry).to_string(),
                    format!("{}i", entry.strip_suffix('y').unwrap_or(entry)),
                ] {
                    let derived = format!("{stem}{suffix}");
                    if derived == entry {
                        continue;
                    }
                    assert!(
                        ALPHABET.binary_search(&derived.as_str()).is_err(),
                        "`{entry}` becomes `{derived}` by adding `{suffix}`, and both are entries"
                    );
                }
            }
        }
    }

    #[test]
    fn every_byte_round_trips() {
        let all: Vec<u8> = (0..=255).collect();
        assert_eq!(decode(&encode(&all)).unwrap(), all);
        assert_eq!(decode_recovered(&encode(&all)).unwrap(), all);
    }

    #[test]
    fn a_single_byte_round_trips_without_separators() {
        let encoded = encode(&[7]);
        assert!(!encoded.contains(' '));
        assert_eq!(decode(&encoded).unwrap(), vec![7]);
    }

    /// The empty case is the one place the bijection does not hold, so it is pinned
    /// rather than left to be discovered.
    #[test]
    fn the_empty_encoding_is_refused_by_both_parsers() {
        assert_eq!(encode(&[]), "");
        assert_eq!(decode(""), Err(DecodeError::Empty));
        assert_eq!(decode_recovered(""), Err(DecodeError::Empty));
        assert_eq!(decode_recovered("   -- \n"), Err(DecodeError::Empty));
        assert_eq!(try_mint(0).unwrap(), "");
    }

    /// The point of the tolerant parser: a value mangled on its way through a model
    /// still yields what was sent.
    #[test]
    fn recovery_survives_the_mangling_a_round_trip_introduces() {
        let bytes = [0x3d, 0x9a, 0x00, 0xff];
        let encoded = encode(&bytes);
        for mangled in [
            encoded.to_uppercase(),
            format!("  {encoded}  "),
            encoded.replace(' ', "-"),
            encoded.replace(' ', ",  "),
            encoded.replace(' ', "\n"),
            format!("\"{}\"", encoded.replace(' ', "   ")),
        ] {
            assert_eq!(decode_recovered(&mangled).unwrap(), bytes, "{mangled}");
        }
    }

    /// And the point of having two: the canonical parser refuses every one of those,
    /// so a boundary that wants one spelling per value can have it.
    #[test]
    fn the_canonical_parser_refuses_what_recovery_accepts() {
        let bytes = [0x3d, 0x9a, 0x00, 0xff];
        let encoded = encode(&bytes);
        for mangled in [
            encoded.to_uppercase(),
            format!("  {encoded}  "),
            encoded.replace(' ', "-"),
            encoded.replace(' ', "  "),
            format!("\"{encoded}\""),
        ] {
            assert!(decode(&mangled).is_err(), "canonical accepted `{mangled}`");
        }
        assert_eq!(decode(&encoded).unwrap(), bytes);
    }

    /// A wrong-case word decodes under recovery and is reported as non-canonical
    /// rather than unknown, because the two call for different responses.
    #[test]
    fn a_case_slip_is_reported_as_non_canonical_not_unknown() {
        let encoded = format!("{} {}", ALPHABET[1], ALPHABET[2].to_uppercase());
        assert_eq!(
            decode(&encoded),
            Err(DecodeError::NotCanonical { position: 1 })
        );
        assert_eq!(decode_recovered(&encoded).unwrap(), vec![1, 2]);
    }

    #[test]
    fn an_unknown_word_is_refused_and_named() {
        let encoded = format!("{} zzzz {}", ALPHABET[1], ALPHABET[2]);
        let expected = Err(DecodeError::UnknownWord {
            position: 1,
            word: "zzzz".to_string(),
        });
        assert_eq!(decode(&encoded), expected);
        assert_eq!(decode_recovered(&encoded), expected);
    }

    /// A near-miss is the case that matters: one character off a real entry must be
    /// refused, not silently read as some other byte.
    #[test]
    fn a_one_character_slip_is_refused_rather_than_read_as_another_byte() {
        assert!(matches!(
            decode_recovered("accesx"),
            Err(DecodeError::UnknownWord { .. })
        ));
    }

    /// Ordinary prose made of alphabet words parses under recovery. Pinned because it
    /// is the price of that parser, and a caller reaching for it should know.
    #[test]
    fn recovery_accepts_ordinary_prose_made_of_alphabet_words() {
        assert!(decode_recovered("error message").is_ok());
        assert!(decode("error message").is_ok());
    }

    #[test]
    fn an_id_round_trips_through_its_canonical_rendering() {
        let id = UnigramId::from_bytes([0x3d, 0x9a, 0x00, 0xff]);
        assert_eq!(id.to_string(), "created office access world");
        assert_eq!(UnigramId::<4>::parse(&id.to_string()).unwrap(), id);
        assert_eq!(
            UnigramId::<4>::recover("CREATED-OFFICE-ACCESS-WORLD").unwrap(),
            id
        );
        assert_eq!(id.as_bytes(), &[0x3d, 0x9a, 0x00, 0xff]);
        assert_eq!(id.into_bytes(), [0x3d, 0x9a, 0x00, 0xff]);
    }

    /// Length is part of the type, so a value that lost or gained a word is refused
    /// on arrival rather than decoding to a shorter id that compares unequal later.
    #[test]
    fn an_id_of_the_wrong_width_is_refused() {
        let five = encode(&[1, 2, 3, 4, 5]);
        assert_eq!(
            UnigramId::<4>::parse(&five),
            Err(ParseError::WrongLength {
                expected: 4,
                found: 5
            })
        );
        assert!(UnigramId::<6>::parse(&five).is_err());
        assert_eq!(
            UnigramId::<5>::parse(&five).unwrap().as_bytes(),
            &[1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn a_random_id_is_the_requested_width_and_not_the_same_twice() {
        let id: UnigramId<8> = UnigramId::try_random().unwrap();
        assert_eq!(id.to_string().split(' ').count(), 8);
        assert_eq!(UnigramId::<8>::parse(&id.to_string()).unwrap(), id);
        assert_ne!(
            UnigramId::<16>::try_random().unwrap(),
            UnigramId::<16>::try_random().unwrap()
        );
    }

    #[test]
    fn minting_produces_one_word_per_requested_byte() {
        let minted = try_mint(4).unwrap();
        assert_eq!(minted.split(' ').count(), 4, "{minted}");
        assert_eq!(decode(&minted).unwrap().len(), 4);
    }

    /// The check word is what turns "we made a valid word unlikely" into "we would
    /// notice". Every one of these mutations decodes fine as a plain UnigramId and
    /// yields the wrong bytes silently.
    #[test]
    fn the_check_word_catches_mutations_the_alphabet_cannot() {
        let id = CheckedUnigramId::from_bytes([17u8, 42, 200, 7]);
        let text = id.to_string();
        assert_eq!(text.split(' ').count(), 5);
        assert_eq!(CheckedUnigramId::<4>::parse(&text).unwrap(), id);
        assert_eq!(id.as_bytes(), &[17u8, 42, 200, 7]);

        let words: Vec<&str> = text.split(' ').collect();
        let other = if words[0] == ALPHABET[0] {
            ALPHABET[1]
        } else {
            ALPHABET[0]
        };
        let substituted = std::iter::once(other)
            .chain(words[1..].iter().copied())
            .collect::<Vec<_>>()
            .join(" ");
        let transposed = {
            let mut w = words.clone();
            w.swap(0, 1);
            w.join(" ")
        };
        for mutation in [substituted, transposed] {
            // The same five words, read as a plain five-byte id, are perfectly
            // valid and yield the wrong bytes without complaint. That is the whole
            // problem, and it is not fixable inside the alphabet.
            assert!(UnigramId::<5>::parse(&mutation).is_ok(), "{mutation}");
            assert!(
                matches!(
                    CheckedUnigramId::<4>::parse(&mutation),
                    Err(ParseError::ChecksumMismatch { .. })
                ),
                "{mutation}"
            );
        }

        // A dropped word changes the count before the checksum is even reached.
        assert!(matches!(
            CheckedUnigramId::<4>::parse(&words[1..].join(" ")),
            Err(ParseError::WrongLength { .. })
        ));
    }

    /// Every single-word substitution is caught, at every position -- swept
    /// exhaustively rather than argued from the 255/256 average.
    #[test]
    fn a_single_word_substitution_is_always_caught() {
        let id = CheckedUnigramId::from_bytes([3u8, 141, 92, 7, 220, 18]);
        let text = id.to_string();
        let words: Vec<&str> = text.split(' ').collect();
        for position in 0..words.len() {
            for replacement in ALPHABET {
                if replacement == words[position] {
                    continue;
                }
                let mut mutated = words.clone();
                mutated[position] = replacement;
                assert!(
                    CheckedUnigramId::<6>::parse(&mutated.join(" ")).is_err(),
                    "substituting `{replacement}` at {position} went unnoticed"
                );
            }
        }
    }

    #[test]
    fn a_checked_value_survives_the_mangling_a_round_trip_introduces() {
        let id: CheckedUnigramId<4> = CheckedUnigramId::try_random().unwrap();
        let text = id.to_string();
        assert_eq!(
            CheckedUnigramId::<4>::recover(&text.to_uppercase().replace(' ', " - ")).unwrap(),
            id
        );
    }

    /// Non-canonical spacing and separators are reported as such, rather than as an
    /// unrecognisable word, because the two call for different responses.
    #[test]
    fn spacing_and_separator_faults_are_reported_as_non_canonical() {
        let good = encode(&[1, 2]);
        for text in [
            format!(" {good}"),
            format!("{good} "),
            good.replace(' ', "  "),
            good.replace(' ', "-"),
            good.to_uppercase(),
        ] {
            assert!(
                matches!(decode(&text), Err(DecodeError::NotCanonical { .. })),
                "`{text}` gave {:?}",
                decode(&text)
            );
        }
        // A word that is nothing at all is still an unknown word.
        assert!(matches!(
            decode("account zzzz"),
            Err(DecodeError::UnknownWord { .. })
        ));
    }

    /// An offending word is attacker-shaped, so it is truncated and escaped before
    /// it reaches a log line.
    #[test]
    fn an_unknown_word_is_previewed_not_echoed() {
        let huge = "q".repeat(10_000);
        let Err(DecodeError::UnknownWord { word, .. }) = decode(&huge) else {
            panic!("expected an unknown word");
        };
        assert!(word.chars().count() <= WORD_PREVIEW + 1, "{}", word.len());

        let Err(DecodeError::UnknownWord { word, .. }) = decode("qqq\u{7}qqq") else {
            panic!("expected an unknown word");
        };
        assert!(!word.contains('\u{7}'), "{word}");
    }

    /// Parsing a fixed width must not size its working memory to its input, or an
    /// enormous value costs memory instead of just time.
    #[test]
    fn a_fixed_width_parse_reports_the_true_length_of_an_overlong_value() {
        let long = encode(&vec![1u8; 5_000]);
        assert_eq!(
            UnigramId::<4>::parse(&long),
            Err(ParseError::WrongLength {
                expected: 4,
                found: 5_000
            })
        );
    }

    /// Neither parser may panic, whatever arrives.
    #[test]
    fn no_input_panics_either_parser() {
        for text in [
            "\u{0}",
            "\u{200b}",
            "🙂",
            "access\u{200b}account",
            &"a".repeat(10_000),
            &"access ".repeat(1_000),
            "-",
            " ",
            "  access  ",
        ] {
            let _ = decode(text);
            let _ = decode_recovered(text);
            let _ = UnigramId::<4>::parse(text);
            let _ = UnigramId::<4>::recover(text);
        }
    }
}
