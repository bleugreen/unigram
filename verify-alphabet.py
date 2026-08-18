# /// script
# requires-python = ">=3.12"
# dependencies = ["ctok>=1.0", "tiktoken", "transformers", "sentencepiece", "protobuf"]
# ///
"""Check every cost claim `unigram` makes, against all five tokenizer families.

The crate itself depends on nothing but the OS CSPRNG and never tokenizes, so
`cargo test` covers only the ALPHABET table's structure and the codec over it.
Everything the crate is NAMED for is checked here instead:

  * every alphabet entry costs exactly one token, in all five families;
  * the property composes -- an encoded value costs one token per byte;
  * that beats the hex it replaces, on the mean and on the worst case;
  * and the space between two words is free, where no other separator is.

    uv run verify-alphabet.py

Exits non-zero and names every offending entry if any of that stops holding. Run
it after ANY edit to the ALPHABET table in src/lib.rs. Requires network access on
first run, to fetch the vocabularies.

Every measurement is MARGINAL -- the cost of appending text to a carrier word,
minus the carrier alone. That is the position an encoded value actually occupies:
inside a message, after something else. Measuring a string on its own would also
charge for whatever frame the family puts around a whole message, and for the
first word opening the string unprefixed, neither of which a byte in a real
encoding pays.
"""

import pathlib
import re
import sys
from typing import Callable, Iterator

LIB = pathlib.Path(__file__).parent / "src" / "lib.rs"
EXPECTED = 256
CARRIER = "the"
SAMPLE_PAYLOAD_SIZES = (4, 8, 16, 32)
SAMPLES = 64


def alphabet() -> list[str]:
    """Parse the ALPHABET table out of the crate source.

    Read from the source rather than duplicated here on purpose: a copy would
    drift, and a checker that verifies a stale copy is worse than none.
    """
    source = LIB.read_text()
    marker = "pub const ALPHABET: [&str; "
    body = source.split(marker, 1)[1].split("= [", 1)[1].split("];", 1)[0]
    return re.findall(r'"([a-z]+)"', body)


def families() -> Iterator[tuple[str, Callable[[str], int]]]:
    """Yield each tokenizer family as a name and a raw token-count function.

    Imported lazily and yielded one at a time so a family that fails to load is
    reported as itself, rather than taking the whole run down before the families
    that did load have said anything.
    """
    import tiktoken

    for name in ("o200k_base", "cl100k_base", "p50k_base", "r50k_base"):
        encoding = tiktoken.get_encoding(name)
        yield name, lambda text, e=encoding: len(e.encode(text))

    from transformers import AutoTokenizer

    llama = AutoTokenizer.from_pretrained("hf-internal-testing/llama-tokenizer")
    yield "llama-sp", lambda text: len(llama.tokenize(text))

    # ctok reconstructs Claude's counts offline; cairn-tokenize is a Rust port of
    # the same model. Unofficial, and not Anthropic's tokenizer -- but it is the
    # only offline counter for the family, and it is exact on every corpus its
    # authors gate against.
    from ctok import token_count

    yield "claude-v5", lambda text: token_count(text, "5.0")


def pseudorandom_bytes(count: int, state: int = 0x2545F4914F6CDD1D) -> Iterator[bytes]:
    """Deterministic payloads, so a run cannot pass or fail on a lucky sample."""
    for _ in range(SAMPLES):
        payload = bytearray()
        while len(payload) < count:
            state ^= (state << 13) & 0xFFFFFFFFFFFFFFFF
            state ^= state >> 7
            state ^= (state << 17) & 0xFFFFFFFFFFFFFFFF
            payload.append((state >> 24) & 0xFF)
        yield bytes(payload)


def check(name: str, raw_count: Callable[[str], int], words: list[str]) -> list[str]:
    """Measure one family against every claim, and return the failures in it."""
    base = raw_count(CARRIER)

    def cost(text: str) -> int:
        """The marginal cost of `text` where an encoded value actually sits."""
        return raw_count(f"{CARRIER} {text}") - base

    def encode(payload: bytes) -> str:
        return " ".join(words[byte] for byte in payload)

    failures: list[str] = []

    expensive = [word for word in words if cost(word) != 1]
    print(f"{name:12s}  {len(words) - len(expensive):3d}/{len(words)} entries are one token")
    failures += [f"{name}: `{word}` is not one token" for word in expensive]

    for size in SAMPLE_PAYLOAD_SIZES:
        hex_costs, word_costs = [], []
        for payload in pseudorandom_bytes(size):
            hex_costs.append(cost(payload.hex()))
            word_costs.append(cost(encode(payload)))
        flat = set(word_costs)
        mean = sum(hex_costs) / len(hex_costs)
        print(
            f"{name:12s}  {size:2d} bytes: unigram {min(word_costs)}-{max(word_costs)}"
            f"  vs hex {mean:.1f} mean / {max(hex_costs)} worst"
        )
        # The invariant, not the average: cost is one token per byte for EVERY
        # value, so an encoded value's size is known before it is minted. Hex cost
        # swings with the value, which is half of what this replaces.
        if flat != {size}:
            failures.append(f"{name}: {size}-byte values cost {sorted(flat)} tokens, not {size}")
        if mean <= size:
            failures.append(f"{name}: hex is no more expensive at {size} bytes ({mean:.1f})")

    # The separator, checked so nobody "tidies" it. A space between two words is
    # absorbed into the space-prefixed vocabulary entry that follows it and costs
    # nothing, in every family. Nothing else is free everywhere -- `_` and `.` are
    # absorbed by the GPT-3.5/4 and GPT-4o vocabularies, and by nothing else -- so
    # the gate is that no alternative UNDERCUTS the space, and that the hyphen, the
    # tempting alternative, is strictly worse.
    payload = bytes(range(1, 9))
    spaced = cost(encode(payload))
    if spaced != len(payload):
        failures.append(f"{name}: spaces are not free -- {len(payload)} words cost {spaced}")
    joined = {
        separator: cost(encode(payload).replace(" ", separator))
        for separator in ("-", "_", ".", ",", "|", "/", "\n")
    }
    print(
        f"{name:12s}  separators: space {spaced}, "
        + ", ".join(f"{separator!r} {c}" for separator, c in joined.items())
    )
    failures += [
        f"{name}: joining on {separator!r} costs {c}, undercutting the space at {spaced}"
        for separator, c in joined.items()
        if c < spaced
    ]
    if joined["-"] <= spaced:
        failures.append(f"{name}: a hyphenated join is no more expensive than a spaced one")

    return failures


def main() -> int:
    words = alphabet()
    if len(words) != EXPECTED:
        print(f"FAIL: expected {EXPECTED} entries, parsed {len(words)}")
        return 1
    if len(set(words)) != len(words):
        print("FAIL: duplicate entries")
        return 1

    failures: list[str] = []
    for name, raw_count in families():
        failures += check(name, raw_count, words)
        print()

    if failures:
        print("FAIL:")
        for failure in failures:
            print(f"  {failure}")
        return 1

    print(f"OK: all {len(words)} entries hold every cost claim, in every family checked.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
