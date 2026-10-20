#!/usr/bin/env python3
"""Regenerate crates/motion-lipsync/data/cmudict.txt.gz from cmusphinx/cmudict `cmudict.dict`.

Usage: build_cmudict_data.py path/to/cmudict.dict output.gz
Keeps the first pronunciation of each lowercase [a-z'] word, strips stress digits,
sorts bytewise, and writes `word PH PH ...` lines gzip-compressed deterministically.
"""
import gzip, re, sys
src, out = sys.argv[1], sys.argv[2]
seen = {}
for line in open(src, encoding="latin-1"):
    line = line.split("#")[0].strip()
    if not line:
        continue
    word, *ph = line.split()
    if re.search(r"\(\d+\)$", word) or not re.fullmatch(r"[a-z][a-z']*", word):
        continue
    if word in seen:
        continue
    seen[word] = " ".join(re.sub(r"\d", "", p) for p in ph)
body = "".join(f"{w} {seen[w]}\n" for w in sorted(seen)).encode()
with open(out, "wb") as f:
    with gzip.GzipFile(fileobj=f, mode="wb", mtime=0, compresslevel=9) as g:
        g.write(body)
print(len(seen), "words", len(body), "bytes raw")
