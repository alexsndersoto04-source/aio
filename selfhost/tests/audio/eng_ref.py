import numpy as np, soundfile as sf, math, sys
f32=np.float32
def sinf(x): return f32(math.sin(float(x)))
def cosf(x): return f32(math.cos(float(x)))
def powf(a,b): return f32(math.pow(float(a),float(b)))
def resample(inp, ch, fr, to):
    if fr==to: return inp.copy()
    n=len(inp); ratio=fr/to; of=max(int(math.ceil(n/ratio)),1)
    out=np.zeros((of,ch),dtype=np.float32)
    for k in range(of):
        pos=k*ratio; i0=min(int(math.floor(pos)),n-1); i1=min(i0+1,n-1)
        frac=f32(pos-math.floor(pos))
        for c in range(ch):
            a=inp[i0,c]; b=inp[i1,c]
            out[k,c]=f32(a+f32(f32(b-a)*frac))
    return out
class BQ:
    def __init__(s,b0,b1,b2,a0,a1,a2):
        if abs(a0)<f32(1e-9): a0=f32(1.0)
        s.b0=f32(b0/a0);s.b1=f32(b1/a0);s.b2=f32(b2/a0);s.a1=f32(a1/a0);s.a2=f32(a2/a0)
        s.x1=s.x2=s.y1=s.y2=f32(0)
    def proc(s,x):
        y=f32(f32(f32(f32(s.b0*x)+f32(s.b1*s.x1))+f32(s.b2*s.x2))-f32(s.a1*s.y1))
        y=f32(y-f32(s.a2*s.y2))
        s.x2=s.x1;s.x1=x;s.y2=s.y1;s.y1=y
        return y
PI=f32(math.pi)
def shelf(kind,freq,rate,db):
    a=powf(10,f32(db/f32(40))); w0=f32(f32(f32(f32(2)*PI)*f32(freq))/f32(rate))
    sn=sinf(w0);cs=cosf(w0); one=f32(1)
    if kind=='peak':
        al=f32(sn/f32(2)); 
        return BQ(f32(one+f32(al*a)),f32(f32(-2)*cs),f32(one-f32(al*a)),f32(one+f32(al/a)),f32(f32(-2)*cs),f32(one-f32(al/a)))
    al=f32(f32(sn/f32(2))*f32(math.sqrt(2))); sqa=f32(math.sqrt(float(a)))
    ap=f32(a+one); am=f32(a-one); t2=f32(f32(f32(2)*sqa)*al)
    if kind=='low':
        return BQ(f32(a*f32(f32(ap-f32(am*cs))+t2)), f32(f32(f32(2)*a)*f32(am-f32(ap*cs))), f32(a*f32(f32(ap-f32(am*cs))-t2)),
                  f32(f32(ap+f32(am*cs))+t2), f32(f32(-2)*f32(am+f32(ap*cs))), f32(f32(ap+f32(am*cs))-t2))
    return BQ(f32(a*f32(f32(ap+f32(am*cs))+t2)), f32(f32(f32(-2)*a)*f32(am+f32(ap*cs))), f32(a*f32(f32(ap+f32(am*cs))-t2)),
              f32(f32(ap-f32(am*cs))+t2), f32(f32(2)*f32(am-f32(ap*cs))), f32(f32(ap-f32(am*cs))-t2))
def run(name, vol, e0,e1,e2, rate_out=48000, och=2):
    data,sr=sf.read(f'eng/{name}.wav',dtype='float32',always_2d=True)
    ch=data.shape[1]
    x=resample(data,ch,sr,rate_out)
    if ch!=och:
        if ch==1: x=np.repeat(x,och,axis=1)
    v=f32(f32(vol)/f32(100))
    x=(x*v).astype(np.float32)
    if (e0,e1,e2)!=(0,0,0):
        bands=[[shelf('low',250,rate_out,e0),shelf('peak',1000,rate_out,e1),shelf('high',4000,rate_out,e2)] for _ in range(och)]
        for n in range(len(x)):
            for c in range(och):
                s=x[n,c]
                for b in bands[c]: s=b.proc(s)
                x[n,c]=s
    return np.clip(x,-1,1).astype(np.float32)
for (n,vol,e0,e1,e2) in [('a48',100,0,0,0),('a48',55,0,0,0),('m22',100,0,0,0),('a48',100,6,-6,6),('s44',80,-3,4,-9)]:
    ref=run(n,vol,e0,e1,e2)
    got=np.fromfile(f'eng/{n}_{vol}_{e0}_{e1}_{e2}.f32',dtype='<f4').reshape(-1,2)
    d=np.abs(got-ref).max() if got.shape==ref.shape else 'shape %s vs %s'%(got.shape,ref.shape)
    print(n,vol,e0,e1,e2,'maxdiff',d,'exact',np.array_equal(got,ref) if got.shape==ref.shape else False)
