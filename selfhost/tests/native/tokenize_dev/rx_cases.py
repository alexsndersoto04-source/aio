# Genera rx_cases.titan (patrones y textos de prueba del motor regex).
BL = r"'s|'t|'re|'ve|'m|'ll|'d| ?\p{L}+| ?\p{N}+| ?[^\s\p{L}\p{N}]+|\s+(?!\S)|\s+"
pats = [BL, r"\w+|[^\w\s]+", r"\s+", r"(?i)'s|'t|'re|'ve", r"[a-c]+?b", r"\d{2,3}", r"(?<=a)b", r"(?<!a)b",
        r"a(?=b)", r"[^\p{L}]+", r"\p{Han}+", r"\p{Greek}|\p{Lu}", r"(?:ab)+c?", r"x*", r"[\p{L}\p{N}_]+", r"\b\w", r"^\s+|\s+$",
        r"(a|ab)(c|bcd)", r"[[a-c][x-z]]+", r"(?s).", r".", r"[^\n]+\n?", r"\s+(?!\S)|\s+", r"(?m)^a", r"a{0}b", r"[\-a]+", r"é+|É", r"(?i)é+", r"(?i)k+", r"(?i)[a-z]+"]
texts = ["Hello, world! It's 12345 don't   stop  ", "  héllo  wörld 日本語 123", "a\nb  c", "abbbcabb", "ab\nab", "ÉéÉ kKK \u212a",
         "αβγ ΑΒΓ Ünï 中文字", "aab ab bab", "", " ", "abcbcd abcd", "x_y 1_2", "   \t\n x"]
def q(s):
    return '"' + s.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n').replace('\t', '\\t').replace('{', '" + LB() + "') + '"'
with open('rx_cases.titan', 'w') as f:
    f.write("fn LB() -> string {\n    \"{\"\n}\n\nfn rx_cases() -> array {\n    let mut a = []\n")
    for p in pats:
        for t in texts:
            f.write(f"    a = std::array::push(a, [{q(p)}, {q(t)}])\n")
    f.write("    a\n}\n")
print(len(pats) * len(texts))
