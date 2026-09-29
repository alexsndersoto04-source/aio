#!/usr/bin/env python3
# Genera selfhost/tests/native/url_casos.titan: muchas URL (WHATWG, IDNA,
# errores) pasadas por todas las funciones de std::url. La salida del
# programa compilado se compara con la VM de Rust (verify_native.sh).
import os
AQUI = os.path.dirname(os.path.abspath(__file__))
SALIDA = os.path.join(AQUI, '..', '..', 'tests', 'native', 'url_casos.titan')

urls = r'''
http://example.com
http://example.com/
HTTP://EXAMPLE.COM/A/B
http://example.com:80/
http://example.com:0080/
http://example.com:8080/x
http://example.com:65535/
http://example.com:65536/
http://example.com:-1/
http://example.com:8a/
http://example.com:/
https://example.com:443/
https://example.com:444/
ws://example.com:80/
wss://example.com:443/
ftp://example.com:21/
ftp://user:pass@example.com/
http://user@example.com/
http://user:@example.com/
http://:pass@example.com/
http://@example.com/
http://a@b@example.com/
http://a:b:c@example.com/
http://us er@example.com/
http://example.com/a b/c
http://example.com/%7e/%7E/%41
http://example.com/./a/../b/./c/..
http://example.com/a/%2e/b/%2E%2e/c
http://example.com/a/.%2e/b
http://example.com/../../x
http://example.com/a/b/c/../../../../d
http://example.com/a?b c
http://example.com/a?b'c
http://example.com/a#b c
http://example.com/a#b`c
http://example.com/a?#
http://example.com/a?
http://example.com/a#
http://example.com?x
http://example.com#x
http://example.com\a\b
http:\\example.com\a
http:/example.com/
http:example.com/
http:///example.com/
http:////example.com/
  http://example.com/  
http://exa	mple.com/
http://example.com/a	b
http://EXAMPLE.com./
http://example..com/
http://.example.com/
http://example.com../
http://ex%41mple.com/
http://ex%2Emple.com/
http://ex%zzample.com/
http://ex%00ample.com/
http://ex%20ample.com/
http://exa<mple.com/
http://exa^mple.com/
http://exa|mple.com/
http://exa%mple.com/
http://[::1]/
http://[::1]:8080/
http://[0:0:0:0:0:0:0:1]/
http://[1:2:3:4:5:6:7:8]/
http://[1:0:0:4:0:0:7:8]/
http://[1:0:0:0:5:0:0:8]/
http://[::]/
http://[1::]/
http://[::ffff:192.168.0.1]/
http://[::ffff:c0a8:1]/
http://[0:0::0:1]/
http://[2001:DB8::1]/
http://[2001:db8:0:0:1:0:0:1]/
http://[1:2:3:4:5:6:7:8:9]/
http://[1::2::3]/
http://[:1]/
http://[1:]/
http://[12345::]/
http://[::1.2.3.4]/
http://[::1.2.3]/
http://[::1.2.3.4.5]/
http://[::1.2.3.256]/
http://[::01.2.3.4]/
http://[g::1]/
http://[::1/
http://::1]/
http://192.168.0.1/
http://192.168.0.257/
http://0x7f.0.0.1/
http://0x7F000001/
http://2130706433/
http://017700000001/
http://0177.0.0.1/
http://127.1/
http://127.0.1/
http://1.2.3.4.5/
http://1.2.3.4./
http://1.2.3.4../
http://4294967295/
http://4294967296/
http://0x100000000/
http://256.256.256.256/
http://1.256/
http://09.1.1.1/
http://0x/
http://0xg.1/
http://1.2.3.09/
http://foo.09/
http://foo.0x4/
http://foo.1.2/
http://a.b.c.1/
file:///etc/passwd
file://localhost/etc/passwd
file://LOCALHOST/etc
file://host/share/file
file:/etc/x
file:etc/x
file:///C:/Windows/../x
file:///C|/x
file:C:/x
file://C:/x
file:///C:/../../x
file:///a/../../b
file://[::1]/x
file:///%41
file:?q
file:#f
file:
file:///
mailto:someone@example.com
mailto:Some One@Example.COM?subject=hi there
data:text/plain,hello world
javascript:alert(1)
urn:isbn:0451450523
news:comp.lang
tel:+1-555-1234
sc://host/path
sc://host:99/path
sc://HOST/path
sc://ho%41st/
sc://h st/
sc://[::1]/
sc://ex ample/
sc:/path
sc:path with space
sc:path#frag ment
sc:path?que ry
sc://
sc:///x
sc://@/
sc://:1/
sc://host:/
sc://ñ/x
sc://Ñ.com/
foo://bar.com:1234/x?y#z
a+b.c-d://x/
1http://x/
http//x
://x
http:
http://
http://?
http://#
http:// /
http://a:b@/
http://a:b@c:9999999/
not a url
x
/relative/path
?query
#frag
http://example.com/😀?😀#😀
http://example.com/\u00e9
http://Bücher.example/
http://BÜCHER.example/
http://xn--bcher-kva.example/
http://XN--BCHER-KVA.example/
http://xn--bcher-kva.XN--example/
http://faß.de/
http://FASS.de/
http://ẞ.de/
http://ﬁ.com/
http://Ａ.com/
http://ｅｘａｍｐｌｅ．ｃｏｍ/
http://a。b。c/
http://例え.テスト/
http://xn--r8jz45g.xn--zckzah/
http://مثال.إختبار/
http://xn--mgbh0fb.xn--kgbechtv/
http://עברית.com/
http://com.עברית/
http://123.עברית/
http://١٢٣.com/
http://a١.com/
http://١a.com/
http://ع1.com/
http://ع١1.com/
http://☃.net/
http://xn--n3h.net/
http://xn--ls8h.la/
http://😀.la/
http://①.com/
http://ǅ.com/
http://𝒜.com/
http://ÀÉ.com/
http://café.com/
http://cafe\u0301.com/
http://\u0301a.com/
http://ᄀ\u1161\u11A8.com/
http://가.com/
http://a\u200Db.com/
http://a\u200Cb.com/
http://\u094D\u200D.com/
http://क\u094D\u200D.com/
http://ب\u200Cب.com/
http://a\u00ADb.com/
http://\u00AD.com/
http://a\u034Fb.com/
http://\uFFFD.com/
http://a\u2028b.com/
http://\u0080.com/
http://xn--.com/
http://xn--a.com/
http://xn--a-.com/
http://xn--zz-.com/
http://xn--99999999999.com/
http://xn--ab-.com/
http://xn--caf-dma.com/
http://xn--cafe-yvc.com/
http://ab--cd.com/
http://a--b.com/
http://-a.com/
http://a-.com/
http://_a.com/
http://a_b.com/
http://ex~ample.com/
http://ex!ample.com/
http://ex$ample.com/
http://ex&ample.com/
http://ex'ample.com/
http://ex(ample.com/
http://ex*ample.com/
http://ex+ample.com/
http://ex,ample.com/
http://ex;ample.com/
http://ex=ample.com/
http://ex"ample.com/
http://ex`ample.com/
http://%E2%98%83.net/
http://%C3%BC.com/
http://%C3.com/
http://%FF.com/
http://B%C3%BCcher.example/
http://xn--1ch.com/
http://Ⅻ.com/
http://ß.com/
http://ς.com/
http://Σ.com/
http://\u0660\u06F0.com/
http://ا\u0660.com/
http://ا\u06F0.com/
http://xn--ngbrx.com/
http://a.b.xn--ngbrx/
http://ab.\u05D0/
http://1.\u05D0/
http://\u05D0\u0300.com/
http://\u05D0\u0300\u0300.com/
http://\u05D0a.com/
http://\u0627\u200C\u0628.com/
http://\u0628\u064B\u200C\u064B\u0628.com/
'''.strip('\n').split('\n')

joins = [
    ('http://a/b/c/d;p?q', 'g:h'), ('http://a/b/c/d;p?q', 'g'), ('http://a/b/c/d;p?q', './g'),
    ('http://a/b/c/d;p?q', 'g/'), ('http://a/b/c/d;p?q', '/g'), ('http://a/b/c/d;p?q', '//g'),
    ('http://a/b/c/d;p?q', '?y'), ('http://a/b/c/d;p?q', 'g?y'), ('http://a/b/c/d;p?q', '#s'),
    ('http://a/b/c/d;p?q', 'g#s'), ('http://a/b/c/d;p?q', ';x'), ('http://a/b/c/d;p?q', ''),
    ('http://a/b/c/d;p?q', '.'), ('http://a/b/c/d;p?q', './'), ('http://a/b/c/d;p?q', '..'),
    ('http://a/b/c/d;p?q', '../'), ('http://a/b/c/d;p?q', '../g'), ('http://a/b/c/d;p?q', '../..'),
    ('http://a/b/c/d;p?q', '../../g'), ('http://a/b/c/d;p?q', '../../../g'),
    ('http://a/b/c/d;p?q', '/./g'), ('http://a/b/c/d;p?q', '/../g'), ('http://a/b/c/d;p?q', 'g.'),
    ('http://a/b/c/d;p?q', '.g'), ('http://a/b/c/d;p?q', 'g..'), ('http://a/b/c/d;p?q', '..g'),
    ('http://a/b/c/d;p?q', 'g?y/./x'), ('http://a/b/c/d;p?q', 'g#s/../x'),
    ('http://a/b/c/d;p?q', 'http:g'), ('http://a/b/c/d;p?q', 'HTTP:g'),
    ('http://a/b/c/d;p?q', '\\\\x\\y'), ('http://a/b/c/d;p?q', '  g  '),
    ('http://example.com/x', 'https://other.org/y'), ('http://example.com/x', '//Bücher.de/p'),
    ('file:///a/b/c', '../d'), ('file:///a/b/c', '/C:/x'), ('file:///C:/a/b', '/x'),
    ('file:///C:/a/b', '..'), ('file://host/a/b', '/x'), ('file:///a', 'file:x'),
    ('file:///a', '//h/x'), ('sc://h/a/b', '../c'), ('sc://h/a/b', 'x'), ('sc:opaque', 'x'),
    ('sc:opaque', '#f'), ('sc:opaque', '?q'), ('mailto:a@b', 'c'), ('not a url', 'x'),
    ('http://a/', 'http://[::1'), ('http://a/b', 'http://exa mple/'), ('http://a/b?c#d', '#e'),
    ('http://a/b?c#d', '?e'), ('http://a/b', '\u00e9'), ('http://a/b', 'x:y'),
    ('http://a/b', 'http:/c'), ('http://a/b', 'http:c'), ('https://a/b', 'http:c'),
    ('http://user:pw@a:81/b', 'c'), ('http://a/b', '//'), ('http://a/b', '///x'),
]

queries = [
    '', '?', 'a=1', '?a=1&b=2', 'a=1&a=2', 'a', 'a=', '=b', '&&&', 'a=1;b=2',
    'a+b=c+d', 'a%20b=c%2Bd', '%zz=%4', '%41%42=%e2%98%83', '%E2%98=x', '%ff=%fe',
    'x=%', 'x=%2', 'é=ü', 'a=b=c', '??a=1', '#a=1', 'a=1#b=2', 'k=v&k=&k',
]

builds = [
    [], [['a', 'b']], [['a b', 'c&d']], [['é', '*-._~']], [['', '']], [['=', '&']],
    [['+', '%']], [['😀', '\t\n']], [['k', 'v'], ['k', 'w']], [['a/b', 'c?d#e']],
]

def tstr(s):
    s = s.replace('\\u', '\x00')
    out = []
    i = 0
    while i < len(s):
        c = s[i]
        if c == '\x00':
            out.append(chr(int(s[i + 1:i + 5], 16)))
            i += 5
            continue
        out.append(c)
        i += 1
    s = ''.join(out)
    assert '{' not in s and '}' not in s
    r = s.replace('\\', '\\\\').replace('"', '\\"').replace('\t', '\\t').replace('\n', '\\n')
    return '"' + r + '"'

o = []
o.append('// GENERADO por native/url/gen_casos.py. Todas las funciones de std::url')
o.append('// con muchas URL (WHATWG, nombres de dominio internacionales y errores).')
o.append('')
o.append('fn show(r: Result) -> string {')
o.append('    match r {')
o.append('        Result::Ok(v) => "{v}",')
o.append('        Result::Err(e) => "Err(" + e + ")",')
o.append('    }')
o.append('}')
o.append('')
o.append('fn one(u: string) {')
o.append('    let v = std::url::is_valid(u)')
o.append('    let s = show(std::try::catch(|| { std::url::scheme(u) }))')
o.append('    let h = show(std::try::catch(|| { std::url::host(u) }))')
o.append('    let p = show(std::try::catch(|| { std::url::port(u) }))')
o.append('    let pa = show(std::try::catch(|| { std::url::path(u) }))')
o.append('    let q = show(std::try::catch(|| { std::url::query(u) }))')
o.append('    let f = show(std::try::catch(|| { std::url::fragment(u) }))')
o.append('    println("[" + u + "] valid={v}")')
o.append('    println("  scheme=" + s + " host=" + h + " port=" + p)')
o.append('    println("  path=" + pa + " query=" + q + " fragment=" + f)')
o.append('}')
o.append('')
o.append('fn une(b: string, r: string) {')
o.append('    let j = show(std::try::catch(|| { std::url::join(b, r) }))')
o.append('    println("join [" + b + "] [" + r + "] = " + j)')
o.append('}')
o.append('')
o.append('fn pq(q: string) {')
o.append('    let m = std::url::parse_query(q)')
o.append('    println("parse_query [" + q + "] = {m}")')
o.append('}')
o.append('')
o.append('fn main() {')
for u in urls:
    o.append('    one(%s)' % tstr(u))
for b, r in joins:
    o.append('    une(%s, %s)' % (tstr(b), tstr(r)))
for q in queries:
    o.append('    pq(%s)' % tstr(q))
for items in builds:
    arr = '[' + ', '.join('[' + ', '.join(tstr(x) for x in it) + ']' for it in items) + ']'
    o.append('    println(std::url::build_query(%s))' % arr)
o.append('}')
open(SALIDA, 'w').write('\n'.join(o) + '\n')
print(len(urls), 'urls', len(joins), 'joins')
