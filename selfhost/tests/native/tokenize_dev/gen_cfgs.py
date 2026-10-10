#!/usr/bin/env python3
"""Genera tokenizers de prueba (con `tokenizers` 0.22.2 de Python) y textos en /tmp/tkcfg.
Uso: PYTHONPATH=<tokenizers==0.22.2> python3 gen_cfgs.py"""
import json, os, copy
from tokenizers import (Regex, Tokenizer, models, normalizers, pre_tokenizers, decoders,
                        processors, trainers, AddedToken)

OUT = "/tmp/tkcfg"
os.makedirs(OUT, exist_ok=True)

CORPUS = [
    "Hello, world! This is a small test of the tokenizer.",
    "The quick brown fox jumps over the lazy dog. Don't stop; it's 2024!",
    "Héllo wörld, café naïve résumé — ünïcödé text with accents.",
    "numbers 12345 and 3.14159, prices $19.99 or 100%.",
    "日本語のテキストと中文文本,还有한국어 텍스트.",
    "emoji 😀 test 🚀 and symbols © ® ™ € £ ¥.",
    "  leading and trailing   spaces  ",
    "tabs\tand\nnewlines\r\nare here",
    "email@example.com http://example.com/path?q=1&r=2",
    "She said: \"I can't, won't, shouldn't, they're, we've, I'm, you'll, he'd.\"",
    "foo bar baz quux Baz FOO Bar",
    "ΑΒΓ αβγ Ωμέγα ΣΊΣΥΦΟΣ straße ǅ İstanbul",
    "a b c d e f g h i j k l m n o p q r s t u v w x y z",
    "the the the of of and and to to in in is is that that it it",
    "e\u0301 a\u0308 n\u0303 ﬁ ﬂ ① ½ ² Å ℌ ㌀",
] * 4

TEXTS = [
    "", " ", "a", "Hello, world!", "  Hello   world  ", "Héllo Wörld café",
    "The quick brown fox jumps over the lazy dog.", "don't can't I'm we've",
    "12345 3.14159 $19.99", "日本語のテキスト 中文文本 한국어", "emoji 😀🚀 ok", "tabs\tand\nnewlines\r\nhere",
    "foo bar baz quux Baz FOO", "foo  bar", "xfoox foo.", " bar ", "BAR baz,quux",
    "ΑΒΓ αβγ ΣΊΣΥΦΟΣ straße İstanbul", "e\u0301 a\u0308 n\u0303 ﬁ ① ½ ² Å ㌀",
    "email@example.com http://example.com/path?q=1&r=2", "unknownzzzword qwxzv", "\u00a0nbsp\u2003em space\u200bzw",
    "a" * 150, "ab" * 40, "<s> </s> <unk> [CLS] [SEP] [MASK] <mask> <|endoftext|>",
    "[CLS] hello [SEP] world", "hello<s>world", "x <mask> y", "x<mask>y", "  <mask>  ", "\x00\x01 ctrl \x7f \ufffd",
    "line1\nline2\n\nline4", "Hello,World!How are you?I'm fine.", "naïve  café   résumé", "ǅ ǆ Ǆ",
    "😀" * 10, "mixed中文and English混合", "﻿BOM start", "tab\t\tdouble", "1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20",
    "the the the of of and and to to", "word. word, word! word? (word) [word] {word}",
]

def train(model, trainer, pre=None, norm=None, files=None):
    tok = Tokenizer(model)
    if norm is not None:
        tok.normalizer = norm
    if pre is not None:
        tok.pre_tokenizer = pre
    tok.train_from_iterator(CORPUS, trainer)
    return tok

def save(name, tok):
    tok.save(f"{OUT}/{name}.json")
    return name

def sid(tok, t):
    return tok.token_to_id(t)

names = []

# 1. BERT (uncased)
tok = train(models.WordPiece(unk_token="[UNK]"),
            trainers.WordPieceTrainer(vocab_size=400, special_tokens=["[PAD]", "[UNK]", "[CLS]", "[SEP]", "[MASK]"]),
            pre=pre_tokenizers.BertPreTokenizer(), norm=normalizers.BertNormalizer(lowercase=True))
tok.post_processor = processors.TemplateProcessing(single="[CLS] $A [SEP]", pair="[CLS] $A [SEP] $B:1 [SEP]:1",
    special_tokens=[("[CLS]", sid(tok, "[CLS]")), ("[SEP]", sid(tok, "[SEP]"))])
tok.decoder = decoders.WordPiece()
names.append(save("bert_uncased", tok))

# 2. BERT (cased) + BertProcessing
tok = train(models.WordPiece(unk_token="[UNK]"),
            trainers.WordPieceTrainer(vocab_size=500, special_tokens=["[PAD]", "[UNK]", "[CLS]", "[SEP]", "[MASK]"]),
            pre=pre_tokenizers.BertPreTokenizer(),
            norm=normalizers.BertNormalizer(lowercase=False, strip_accents=False, clean_text=True, handle_chinese_chars=True))
tok.post_processor = processors.BertProcessing(("[SEP]", sid(tok, "[SEP]")), ("[CLS]", sid(tok, "[CLS]")))
tok.decoder = decoders.WordPiece(prefix="##", cleanup=False)
names.append(save("bert_cased", tok))

# 2b. BERT con strip_accents sin lowercase, sin chinese, con padding y truncation
tok = train(models.WordPiece(unk_token="[UNK]", max_input_chars_per_word=20),
            trainers.WordPieceTrainer(vocab_size=500, special_tokens=["[PAD]", "[UNK]", "[CLS]", "[SEP]", "[MASK]"]),
            pre=pre_tokenizers.BertPreTokenizer(),
            norm=normalizers.BertNormalizer(lowercase=False, strip_accents=True, clean_text=False, handle_chinese_chars=False))
tok.post_processor = processors.TemplateProcessing(single="[CLS] $A [SEP]", pair="[CLS] $A [SEP] $B:1 [SEP]:1",
    special_tokens=[("[CLS]", sid(tok, "[CLS]")), ("[SEP]", sid(tok, "[SEP]"))])
tok.enable_truncation(max_length=10)
tok.enable_padding(length=14, pad_id=0, pad_token="[PAD]")
names.append(save("bert_trunc_pad", tok))

tok2 = Tokenizer.from_file(f"{OUT}/bert_uncased.json")
tok2.enable_truncation(max_length=7, direction="left", stride=0, strategy="only_first")
tok2.enable_padding(direction="left", pad_to_multiple_of=4, pad_id=0, pad_token="[PAD]")
names.append(save("bert_trunc_left", tok2))

tok2 = Tokenizer.from_file(f"{OUT}/bert_uncased.json")
tok2.enable_truncation(max_length=2)
tok2.enable_padding(length=3, pad_id=7, pad_type_id=2, pad_token="[UNK]")
names.append(save("bert_trunc_small", tok2))

# 3. GPT-2
tok = train(models.BPE(), trainers.BpeTrainer(vocab_size=600, initial_alphabet=pre_tokenizers.ByteLevel.alphabet(),
            special_tokens=["<|endoftext|>"]), pre=pre_tokenizers.ByteLevel(add_prefix_space=False))
tok.decoder = decoders.ByteLevel()
tok.post_processor = processors.ByteLevel(trim_offsets=True)
names.append(save("gpt2", tok))

# 3b. ByteLevel sin regex, con prefix space
tok = train(models.BPE(), trainers.BpeTrainer(vocab_size=500, initial_alphabet=pre_tokenizers.ByteLevel.alphabet()),
            pre=pre_tokenizers.ByteLevel(add_prefix_space=True, use_regex=False))
tok.decoder = decoders.ByteLevel()
names.append(save("bytelevel_noregex", tok))

# 4. RoBERTa
tok = train(models.BPE(), trainers.BpeTrainer(vocab_size=600, initial_alphabet=pre_tokenizers.ByteLevel.alphabet(),
            special_tokens=["<s>", "<pad>", "</s>", "<unk>", "<mask>"]), pre=pre_tokenizers.ByteLevel(add_prefix_space=True))
tok.decoder = decoders.ByteLevel()
tok.post_processor = processors.RobertaProcessing(("</s>", sid(tok, "</s>")), ("<s>", sid(tok, "<s>")), trim_offsets=True, add_prefix_space=True)
tok.add_special_tokens([AddedToken("<mask2>", lstrip=True, special=True)])
names.append(save("roberta", tok))

# 5. Llama-like (byte fallback)
bytes_toks = [f"<0x{b:02X}>" for b in range(256)]
tok = train(models.BPE(unk_token="<unk>", fuse_unk=True, byte_fallback=True),
            trainers.BpeTrainer(vocab_size=700, special_tokens=["<unk>", "<s>", "</s>"] + bytes_toks),
            norm=normalizers.Sequence([normalizers.Prepend("▁"), normalizers.Replace(" ", "▁")]))
tok.decoder = decoders.Sequence([decoders.Replace("▁", " "), decoders.ByteFallback(), decoders.Fuse(), decoders.Strip(" ", 1, 0)])
tok.post_processor = processors.TemplateProcessing(single="<s> $A", pair="<s> $A <s> $B:1",
    special_tokens=[("<s>", sid(tok, "<s>"))])
names.append(save("llama_like", tok))

# 6. Metaspace + BPE con unk (sin fuse)
for scheme in ("always", "first", "never"):
    tok = train(models.BPE(unk_token="<unk>"), trainers.BpeTrainer(vocab_size=400, special_tokens=["<unk>"]),
                pre=pre_tokenizers.Metaspace(replacement="▁", prepend_scheme=scheme))
    tok.decoder = decoders.Metaspace(replacement="▁", prepend_scheme=scheme)
    names.append(save(f"metaspace_{scheme}", tok))

# 6b. Metaspace con normalizador Strip (la regla `first` depende del desplazamiento)
tok = train(models.BPE(unk_token="<unk>"), trainers.BpeTrainer(vocab_size=400, special_tokens=["<unk>"]),
            pre=pre_tokenizers.Metaspace(replacement="▁", prepend_scheme="first", split=False),
            norm=normalizers.Strip(left=True, right=True))
names.append(save("metaspace_first_strip", tok))

# 7. WordLevel + Whitespace
tok = train(models.WordLevel(unk_token="[UNK]"), trainers.WordLevelTrainer(vocab_size=200, special_tokens=["[UNK]", "[PAD]"]),
            pre=pre_tokenizers.Whitespace())
names.append(save("wordlevel_ws", tok))

# 7b. WordLevel con WhitespaceSplit + lowercase
tok = train(models.WordLevel(unk_token="[UNK]"), trainers.WordLevelTrainer(vocab_size=200, special_tokens=["[UNK]"]),
            pre=pre_tokenizers.WhitespaceSplit(), norm=normalizers.Lowercase())
names.append(save("wordlevel_split", tok))

# 8. Sequence pre-tokenizers
tok = train(models.BPE(unk_token="<unk>"), trainers.BpeTrainer(vocab_size=500, special_tokens=["<unk>"]),
            pre=pre_tokenizers.Sequence([pre_tokenizers.Digits(individual_digits=True), pre_tokenizers.Punctuation(),
                                         pre_tokenizers.WhitespaceSplit()]))
names.append(save("seq_digits_punct", tok))

tok = train(models.BPE(unk_token="<unk>"), trainers.BpeTrainer(vocab_size=500, special_tokens=["<unk>"]),
            pre=pre_tokenizers.Sequence([pre_tokenizers.Digits(individual_digits=False),
                                         pre_tokenizers.Punctuation(behavior="contiguous")]))
names.append(save("seq_digits_contig", tok))

for beh, inv in (("removed", False), ("isolated", False), ("merged_with_previous", False),
                 ("merged_with_next", False), ("contiguous", False), ("isolated", True), ("removed", True)):
    tok = train(models.BPE(unk_token="<unk>"), trainers.BpeTrainer(vocab_size=400, special_tokens=["<unk>"]),
                pre=pre_tokenizers.Split(pattern=Regex(r"\s+|[,.!?]"), behavior=beh, invert=inv))
    names.append(save(f"split_{beh}_{'inv' if inv else 'dir'}", tok))

tok = train(models.BPE(unk_token="<unk>"), trainers.BpeTrainer(vocab_size=400, special_tokens=["<unk>"]),
            pre=pre_tokenizers.Split(pattern="o", behavior="isolated"))
names.append(save("split_string", tok))

tok = train(models.BPE(unk_token="<unk>"), trainers.BpeTrainer(vocab_size=400, special_tokens=["<unk>"]),
            pre=pre_tokenizers.Sequence([pre_tokenizers.CharDelimiterSplit(","), pre_tokenizers.WhitespaceSplit()]))
names.append(save("delimiter", tok))

# 9. Normalizadores
tok = train(models.BPE(unk_token="<unk>"), trainers.BpeTrainer(vocab_size=500, special_tokens=["<unk>"]),
            pre=pre_tokenizers.WhitespaceSplit(),
            norm=normalizers.Sequence([normalizers.NFD(), normalizers.StripAccents(), normalizers.Lowercase(),
                                       normalizers.Strip(), normalizers.Replace(Regex(r"\d+"), "#")]))
names.append(save("norm_seq1", tok))
for nm, n in (("nfc", normalizers.NFC()), ("nfkc", normalizers.NFKC()), ("nfkd", normalizers.NFKD()), ("nfd", normalizers.NFD()),
              ("nmt", normalizers.Nmt()), ("stripl", normalizers.Strip(left=True, right=False)),
              ("stripr", normalizers.Strip(left=False, right=True)),
              ("prepend", normalizers.Prepend(">> ")), ("replace_str", normalizers.Replace("e", "EE")),
              ("replace_empty_rx", normalizers.Replace(Regex(r"x*"), "-")),
              ("bytelevel_norm", normalizers.ByteLevel()), ("lower", normalizers.Lowercase()),
              ("seq_nmt_nfkc", normalizers.Sequence([normalizers.Nmt(), normalizers.NFKC()])),
              ("stripacc", normalizers.StripAccents())):
    tok = train(models.BPE(unk_token="<unk>"), trainers.BpeTrainer(vocab_size=400, special_tokens=["<unk>"]),
                pre=pre_tokenizers.WhitespaceSplit(), norm=n)
    names.append(save(f"norm_{nm}", tok))

# 10. Added tokens con opciones
tok = train(models.BPE(unk_token="<unk>"), trainers.BpeTrainer(vocab_size=400, special_tokens=["<unk>"]),
            pre=pre_tokenizers.Whitespace(), norm=normalizers.Lowercase())
tok.add_tokens([AddedToken("foo", single_word=True), AddedToken("bar", lstrip=True, rstrip=True),
                AddedToken("Baz", normalized=False), AddedToken("quux", normalized=True), AddedToken("héllo", normalized=True)])
tok.add_special_tokens([AddedToken("[MASK]", lstrip=True, special=True), AddedToken("<mask>", rstrip=True, special=True),
                        AddedToken("[CLS]", special=True), AddedToken("[SEP]", special=True, normalized=True)])
tok.decoder = decoders.BPEDecoder(suffix="</w>")
names.append(save("added_opts", tok))

# 11. BPE con prefix/suffix
tok = train(models.BPE(unk_token="<unk>", continuing_subword_prefix="##", end_of_word_suffix="</w>"),
            trainers.BpeTrainer(vocab_size=400, special_tokens=["<unk>"], continuing_subword_prefix="##", end_of_word_suffix="</w>"),
            pre=pre_tokenizers.Whitespace())
tok.decoder = decoders.BPEDecoder(suffix="</w>")
names.append(save("bpe_affix", tok))

# 11b. ignore_merges, fuse_unk false
tok = train(models.BPE(unk_token="<unk>", ignore_merges=True), trainers.BpeTrainer(vocab_size=400, special_tokens=["<unk>"]),
            pre=pre_tokenizers.Whitespace())
names.append(save("bpe_ignore", tok))
tok = train(models.BPE(unk_token="<unk>", fuse_unk=True), trainers.BpeTrainer(vocab_size=100, special_tokens=["<unk>"]),
            pre=pre_tokenizers.Whitespace())
names.append(save("bpe_fuse", tok))
tok = train(models.BPE(), trainers.BpeTrainer(vocab_size=120), pre=pre_tokenizers.Whitespace())
names.append(save("bpe_no_unk", tok))
tok = train(models.BPE(unk_token="<unk>"), trainers.BpeTrainer(vocab_size=120, special_tokens=["<unk>"]), pre=pre_tokenizers.Whitespace())
names.append(save("bpe_unk_nofuse", tok))

# 12. FixedLength / decoders por JSON
tok = train(models.BPE(unk_token="<unk>"), trainers.BpeTrainer(vocab_size=400, special_tokens=["<unk>"]),
            pre=pre_tokenizers.WhitespaceSplit())
j = json.loads(tok.to_str())
j["pre_tokenizer"] = {"type": "FixedLength", "length": 3}
open(f"{OUT}/fixedlen.json", "w").write(json.dumps(j))
names.append("fixedlen")

def with_decoder(base, name, dec, post=None, extra=None):
    j = json.loads(open(f"{OUT}/{base}.json").read())
    j["decoder"] = dec
    if post is not None:
        j["post_processor"] = post
    if extra:
        extra(j)
    open(f"{OUT}/{name}.json", "w").write(json.dumps(j))
    names.append(name)

with_decoder("bert_uncased", "dec_ctc", {"type": "CTC", "pad_token": "[PAD]", "word_delimiter_token": "[SEP]", "cleanup": True})
with_decoder("bert_uncased", "dec_ctc2", {"type": "CTC", "pad_token": "<pad>", "word_delimiter_token": "|", "cleanup": False})
with_decoder("bert_uncased", "dec_wp_nocleanup", {"type": "WordPiece", "prefix": "##", "cleanup": True})
with_decoder("bert_uncased", "dec_strip", {"type": "Strip", "content": "~", "start": 1, "stop": 1})
with_decoder("bert_uncased", "dec_replace_rx", {"type": "Sequence", "decoders": [
    {"type": "Replace", "pattern": {"Regex": "l+"}, "content": "L"}, {"type": "Fuse"}]})
with_decoder("bert_uncased", "dec_bpe", {"type": "BPEDecoder", "suffix": "##"})
with_decoder("bert_uncased", "dec_fuse", {"type": "Fuse"})
with_decoder("bert_uncased", "dec_none", None)
with_decoder("bert_uncased", "post_seq", None, post={"type": "Sequence", "processors": [
    {"type": "ByteLevel", "add_prefix_space": True, "trim_offsets": True, "use_regex": True},
    {"type": "TemplateProcessing", "single": [{"SpecialToken": {"id": "[CLS]", "type_id": 0}}, {"Sequence": {"id": "A", "type_id": 0}},
        {"SpecialToken": {"id": "[SEP]", "type_id": 0}}, {"SpecialToken": {"id": "[SEP]", "type_id": 1}}],
     "pair": [], "special_tokens": {"[CLS]": {"id": "[CLS]", "ids": [2], "tokens": ["[CLS]"]}, "[SEP]": {"id": "[SEP]", "ids": [3], "tokens": ["[SEP]"]}}},
    {"type": "BertProcessing", "sep": ["[SEP]", 3], "cls": ["[CLS]", 2]}]})
with_decoder("bert_uncased", "post_template_multi", None, post={"type": "TemplateProcessing",
    "single": [{"SpecialToken": {"id": "<a>", "type_id": 0}}, {"Sequence": {"id": "A", "type_id": 1}}, {"SpecialToken": {"id": "<b>", "type_id": 2}}],
    "pair": [], "special_tokens": {"<a>": {"id": "<a>", "ids": [5, 6], "tokens": ["x", "y"]}, "<b>": {"id": "<b>", "ids": [7], "tokens": ["z"]}}})
with_decoder("roberta", "post_seq_roberta", None, post={"type": "Sequence", "processors": [
    {"type": "TemplateProcessing", "single": [{"SpecialToken": {"id": "<s>", "type_id": 0}}, {"Sequence": {"id": "A", "type_id": 3}}],
     "pair": [], "special_tokens": {"<s>": {"id": "<s>", "ids": [0], "tokens": ["<s>"]}}},
    {"type": "RobertaProcessing", "sep": ["</s>", 2], "cls": ["<s>", 0], "trim_offsets": True, "add_prefix_space": True}]})


# 13. Unigram
tok = train(models.Unigram(), trainers.UnigramTrainer(vocab_size=300, special_tokens=["<unk>", "<s>", "</s>"], unk_token="<unk>"),
            pre=pre_tokenizers.Metaspace(), norm=normalizers.Sequence([normalizers.NFKC(), normalizers.Lowercase()]))
tok.decoder = decoders.Metaspace()
tok.post_processor = processors.TemplateProcessing(single="$A </s>", pair="$A </s> $B:1 </s>:1", special_tokens=[("</s>", sid(tok, "</s>"))])
names.append(save("unigram_basic", tok))

def patch_model(base, name, fn):
    j = json.loads(open(f"{OUT}/{base}.json").read())
    fn(j["model"])
    open(f"{OUT}/{name}.json", "w").write(json.dumps(j))
    names.append(name)

def add_bytes(m):
    m["vocab"] += [[f"<0x{b:02X}>", 0.0] for b in range(256)]
    m["byte_fallback"] = True
patch_model("unigram_basic", "unigram_bytefb", add_bytes)
def no_unk(m):
    m["unk_id"] = None
patch_model("unigram_basic", "unigram_nounk", no_unk)
def dup_tok(m):
    m["vocab"] += [["▁the", -3.5], ["▁the", -1.5], ["e", -9]]
patch_model("unigram_basic", "unigram_dup", dup_tok)
tok = train(models.Unigram(), trainers.UnigramTrainer(vocab_size=200, special_tokens=["<unk>"], unk_token="<unk>"),
            pre=pre_tokenizers.Sequence([pre_tokenizers.WhitespaceSplit()]))
names.append(save("unigram_ws", tok))

import random
rnd = random.Random(12345)
POOL = list("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 .,;:!?'\"()-_/\\@#$%^&*+=<>[]{}~`|") + \
    [" ", "  ", "\t", "\n", "\r\n", "\u00a0", "\u2003", "\u200b", "\u3000", "é", "è", "ñ", "ü", "ß", "ø", "å", "ç", "e\u0301", "a\u0308",
     "Σ", "σ", "ς", "Ω", "İ", "ı", "ǅ", "ﬁ", "①", "½", "²", "Å", "㌀", "中", "文", "日", "本", "語", "한", "국", "あ", "ア", "😀", "🚀", "👍🏽", "\u0000", "\u0007", "\ufffd", "\ufeff",
     "foo", "bar", "Baz", "quux", "héllo", "<mask>", "[MASK]", "[CLS]", "<s>", "ing", "tion", "the", "and", "'s", "n't", "▁", "##"]
for _ in range(220):
    TEXTS.append("".join(rnd.choice(POOL) for _ in range(rnd.randint(1, 40))))

json.dump(names, open(f"{OUT}/list.json", "w"))
json.dump(TEXTS, open(f"{OUT}/texts.json", "w"))
print(len(names), "tokenizers", len(TEXTS), "textos")
