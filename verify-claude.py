# /// script
# requires-python = ">=3.12"
# dependencies = ["anthropic>=0.40", "ctok==1.0.0"]
# ///
"""Cross-check the Claude column against Anthropic's official token counter.

`verify-alphabet.py` measures Claude with `ctok`, an unofficial offline
reconstruction. That is the weakest source behind the crate's strongest claim --
Claude is where `unigram` beats the compact encodings outright -- so this script
checks the same properties against `POST /v1/messages/count_tokens`, which is
authoritative, and reports where `ctok` and the API disagree.

    export ANTHROPIC_API_KEY=sk-ant-...
    uv run verify-claude.py            # spaced claim + the cost table (~40 calls)
    uv run verify-claude.py --bare     # also every entry bare (~300 calls total)

Counts are model-specific, so the model is named rather than assumed. Opus 4.7
and later share a tokenizer; `ctok`'s "5.0" targets that family.
"""

import os
import pathlib
import re
import sys
from concurrent.futures import ThreadPoolExecutor

from anthropic import Anthropic
from ctok import token_count

MODEL = "claude-opus-5"
LIB = pathlib.Path(__file__).parent / "src" / "lib.rs"
CARRIER = "the"
CHUNK = 8
WORKERS = 8


def alphabet() -> list[str]:
    source = LIB.read_text()
    body = source.split("pub const ALPHABET: [&str; ", 1)[1].split("= [", 1)[1].split("];", 1)[0]
    return re.findall(r'"([a-z]+)"', body)


def main() -> int:
    if not os.environ.get("ANTHROPIC_API_KEY"):
        print("ANTHROPIC_API_KEY is not set. See the module docstring.")
        return 2
    client = Anthropic()
    words = alphabet()

    def official(text: str) -> int:
        return client.messages.count_tokens(
            model=MODEL, messages=[{"role": "user", "content": text}]
        ).input_tokens

    def ctok_count(text: str) -> int:
        return token_count(text, "5.0")

    # The per-message frame, subtracted from everything so only the text is compared.
    frame_api, frame_ctok = official(CARRIER), ctok_count(CARRIER)
    print(f"model {MODEL}   frame: api {frame_api}, ctok {frame_ctok}\n")

    failures: list[str] = []

    # Spaced cost, in chunks. A chunk of 8 words that costs 8 tokens marginal
    # proves all 8 are single-token; chunking rather than one 256-word call keeps a
    # failure localised to eight candidates instead of the whole table.
    print(f"spaced cost, {len(words) // CHUNK} chunks of {CHUNK}")
    chunks = [words[i : i + CHUNK] for i in range(0, len(words), CHUNK)]

    def check_chunk(chunk: list[str]) -> tuple[list[str], int, int]:
        joined = " ".join(chunk)
        return chunk, official(f"{CARRIER} {joined}") - frame_api, ctok_count(f"{CARRIER} {joined}") - frame_ctok

    with ThreadPoolExecutor(WORKERS) as pool:
        for chunk, api, ctok_marginal in pool.map(check_chunk, chunks):
            if api != CHUNK:
                failures.append(f"spaced: {chunk} costs {api} tokens, not {CHUNK}")
            if api != ctok_marginal:
                failures.append(
                    f"ctok disagrees on {chunk}: api {api}, ctok {ctok_marginal}"
                )
    print(f"  {len(chunks) - len([f for f in failures if f.startswith('spaced')])}"
          f"/{len(chunks)} chunks cost exactly {CHUNK}\n")

    # The published cost table.
    print("cost table (mean over 64 deterministic payloads is in verify-alphabet.py;")
    print("this checks the flat per-byte claim on one payload per size)")
    state = 0x2545F4914F6CDD1D
    for size in (4, 8, 16, 32):
        payload = bytearray()
        while len(payload) < size:
            state ^= (state << 13) & 0xFFFFFFFFFFFFFFFF
            state ^= state >> 7
            state ^= (state << 17) & 0xFFFFFFFFFFFFFFFF
            payload.append((state >> 24) & 0xFF)
        value = " ".join(words[b] for b in payload)
        api = official(f"{CARRIER} {value}") - frame_api
        ctok_marginal = ctok_count(f"{CARRIER} {value}") - frame_ctok
        hex_api = official(f"{CARRIER} {bytes(payload).hex()}") - frame_api
        flag = "" if api == size else f"   <-- expected {size}"
        print(f"  {size:2d} bytes: unigram api {api:3d} / ctok {ctok_marginal:3d}"
              f"   hex api {hex_api:3d}{flag}")
        if api != size:
            failures.append(f"a {size}-byte value costs {api} tokens under the API, not {size}")
        if api != ctok_marginal:
            failures.append(f"ctok disagrees at {size} bytes: api {api}, ctok {ctok_marginal}")

    # Bare cost, one call per entry. This is the property 0.2.0 got wrong, so it is
    # worth paying for -- but it is 256 calls, hence opt-in.
    if "--bare" in sys.argv:
        print(f"\nbare cost, {len(words)} calls")

        def check_bare(word: str) -> tuple[str, int, int]:
            return word, official(word) - official(""), ctok_count(word) - ctok_count("")

        with ThreadPoolExecutor(WORKERS) as pool:
            over = []
            for word, api, ctok_marginal in pool.map(check_bare, words):
                if api != 1:
                    over.append(word)
                    failures.append(f"bare: `{word}` costs {api} tokens under the API")
                if api != ctok_marginal:
                    failures.append(f"ctok disagrees on bare `{word}`: api {api}, ctok {ctok_marginal}")
        print(f"  {len(words) - len(over)}/{len(words)} entries are one token bare")
    else:
        print("\nbare cost skipped; pass --bare to check all 256 (one call each)")

    if failures:
        print(f"\nFAIL ({len(failures)}):")
        for failure in failures[:40]:
            print(f"  {failure}")
        if len(failures) > 40:
            print(f"  ... and {len(failures) - 40} more")
        return 1
    print("\nOK: the official counter agrees with ctok on everything checked.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
