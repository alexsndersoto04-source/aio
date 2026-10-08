import subprocess, sys
bases = [r'[a-b]', 'a', r'\w', r'\d', r'\pL', '.', '(?s:.)', '(a)', '(?:ab|cd)', '[^a]', r'\b', '(?i)k', 'é', '[α-ω]', r'\W',
 r'(?-u:\w)', r'\p{Greek}', r'\S', 'a?', 'a*', '(?:a|b|c)+', 'x{2,5}', 'foo|bar|baz', r'\p{Han}', r'[\x{10000}-\x{10FFFF}]',
 '^a', 'a$', '(?m)^a$', r'\w+?', '(?U)a*', '[a-z]{0,3}', r'\p{any}', r'\w\d', '[[:alpha:]]', 'ab|ab', r'(?i)\w', r'\p{Lu}',
 'abc|abd|xyz', r'(?:(a)|b)', r'[\x{80}-\x{7FF}]', r'[\x{800}-\x{FFFF}]', r'(?i)[a-zé]', r'\B', r'(?-u)\b', 'a|', r'(?s).*?']
open('b.txt','w').write('\n'.join(b.encode().hex() for b in bases)+'\n')
r = subprocess.run(['zett','run','umbral.titan','b.txt'],capture_output=True,text=True)
ths = r.stdout.split()
assert len(ths)==len(bases), r.stderr
pats=[]
for b,t in zip(bases,ths):
    t=int(t)
    for n in (t-1,t,t+1):
        if n>=0: pats.append((b,t,n,'(?:%s){%d}'%(b,n)))
open('p.txt','w').write('\n'.join(p[3].encode().hex() for p in pats)+'\n')
out = subprocess.run([sys.argv[1],'p.txt'],capture_output=True,text=True).stdout.split('\n')
bad=0
for (b,t,n,p),o in zip(pats,out):
    exp = 'OK' if n<=t or t==3999999 else 'Compiled regex exceeds size limit of 10485760 bytes.'
    got = 'OK' if o=='OK' else bytes.fromhex(o).decode()
    if got!=exp:
        bad+=1; print('DIFF',repr(b),t,n,repr(got[:80]))
print('bases',len(bases),'pruebas',len(pats),'diferencias',bad)
print(' '.join('%s=%s'%(b,t) for b,t in zip(bases,ths)))
