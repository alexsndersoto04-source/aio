import re
L='/tmp/cr/zstd-sys-2.0.16+zstd.1.5.7/zstd/lib/'
src=open(L+'compress/zstd_compress_internal.h').read()+open(L+'common/zstd_internal.h').read()+open(L+'compress/zstd_compress_sequences.c').read()+open(L+'compress/fse_compress.c').read()
def arr(name):
    m=re.search(r'\b'+name+r'\[[^\]]*\]\s*=\s*\{([^}]*)\}',src)
    body=re.sub(r'/\*.*?\*/','',m.group(1),flags=re.S)
    return [int(x) for x in body.replace('\n',' ').split(',') if x.strip()]
tabs=[('LL_Code',64),('ML_Code',128),('LL_bits',36),('ML_bits',53),('LL_defaultNorm',36),('ML_defaultNorm',53),('OF_defaultNorm',29),('kInverseProbabilityLog256',256),('rtbTable',8)]
out=['// Tablas constantes de libzstd 1.5.7 (generado por native/compress/gen_zstd_enc_tab.py',
     '// desde zstd_compress_internal.h, zstd_internal.h, zstd_compress_sequences.c',
     '// y fse_compress.c; no editar a mano). Palabras de 8 bytes:']
off=0
lay=[]
for n,c in tabs:
    a=arr(n); assert len(a)==c,(n,len(a))
    lay.append((n,off,a)); off+=8*c
cl=open(L+'compress/clevels.h').read()
strat={'ZSTD_fast':1,'ZSTD_dfast':2,'ZSTD_greedy':3,'ZSTD_lazy':4,'ZSTD_lazy2':5,'ZSTD_btlazy2':6,'ZSTD_btopt':7,'ZSTD_btultra':8,'ZSTD_btultra2':9}
rows=re.findall(r'\{\s*(\d+),\s*(\d+),\s*(\d+),\s*(\d+),\s*(\d+),\s*(\d+),\s*(ZSTD_\w+)\s*\}',cl)
assert len(rows)==92,len(rows)
clv=[]
for r in rows: clv+= [int(x) for x in r[:6]]+[strat[r[6]]]
lay.append(('clevels[4][23][W,C,H,S,L,TL,strat]',off,clv)); off+=8*len(clv)
out.append('//   '+'  '.join('%d %s'%(o,n) for n,o,a in lay))
out.append('')
out.append('fn ze_tables() -> int {')
out.append('    let t = rt_alloc(%d)'%off)
out.append('    let mut i = 0')
for n,o,a in lay:
    items=[str(x) if x>=0 else '0 - %d'%(-x) for x in a]
    lines=[]; cur='        '
    for it in items:
        s=it+', '
        if len(cur)+len(s)>88: lines.append(cur.rstrip()); cur='        '
        cur+=s
    lines.append(cur.rstrip().rstrip(','))
    out.append('    let a%d = ['%o)
    out+=lines
    out.append('    ]')
    out.append('    i = 0')
    out.append('    while i < %d {'%len(a))
    out.append('        std::raw::store(t + %d + 8 * i, a%d[i])'%(o,o))
    out.append('        i += 1')
    out.append('    }')
out.append('    t')
out.append('}')
open('/home/user/aio/selfhost/native/std_compress_zstd_enc_tab.titan','w').write('\n'.join(out)+'\n')
print(off, [(n,o) for n,o,a in lay])
