import os,io,tarfile,gzip,hashlib,base64,json,subprocess,sys,shutil
data=sys.argv[1]; port=sys.argv[2]
seed=bytes(range(1,33))
shutil.rmtree(data,ignore_errors=True)
os.makedirs(data+"/index"); os.makedirs(data+"/archive")
open(data+"/seed.pem","w").write("-----BEGIN PRIVATE KEY-----\n"+base64.encodebytes(bytes.fromhex("302e020100300506032b657004220420")+seed).decode()+"-----END PRIVATE KEY-----\n")
pub=subprocess.run(["openssl","pkey","-in",data+"/seed.pem","-pubout","-outform","DER"],capture_output=True).stdout[-32:]
def sign(dig):
    open(data+"/d.bin","wb").write(dig)
    subprocess.run(["openssl","pkeyutl","-sign","-inkey",data+"/seed.pem","-rawin","-in",data+"/d.bin","-out",data+"/s.bin"],check=True)
    return open(data+"/s.bin","rb").read()
def tpkg(name,version,deps):
    toml='[package]\nname="%s"\nversion="%s"\nedition="2021"\n[dependencies]\n'%(name,version)
    bio=io.BytesIO()
    with tarfile.open(fileobj=bio,mode="w",format=tarfile.GNU_FORMAT) as t:
        for n,b in (("Titan.toml",toml.encode()),("src/lib.titan",("fn %s_v(){ print(\"%s\") }\n"%(name,version)).encode())):
            ti=tarfile.TarInfo(n); ti.size=len(b); t.addfile(ti,io.BytesIO(b))
    return gzip.compress(bio.getvalue(),mtime=0)
index={}
def add(name,version,deps,badsig=False):
    gz=tpkg(name,version,deps)
    sha=hashlib.sha256(gz).hexdigest()
    fn="%s-%s.tpkg"%(name,version)
    open(data+"/archive/"+fn,"wb").write(gz)
    sig=sign(hashlib.sha256(gz).digest() if not badsig else b"\0"*32)
    index.setdefault(name,[]).append({"version":version,"archive":"https://localhost:%s/archive/%s"%(port,fn),"sha256":sha,"signing_key":base64.b64encode(pub).decode(),"signature":base64.b64encode(sig).decode(),"dependencies":deps})
add("alpha","1.0.0",{"beta":"^1"})
add("alpha","1.1.0",{"beta":"^2"})
add("beta","1.0.0",{})
add("beta","1.5.0",{})
add("beta","2.0.0",{})
add("beta","2.1.0-rc.1",{})
add("gamma","0.3.0",{})
add("gamma","0.3.5",{})
add("gamma","not-a-version",{})
add("delta","1.0.0",{},badsig=True)
add("eps","1.0.0",{"zzz":"^1"})
add("looper","1.0.0",{"looper2":"^1"})
add("looper2","1.0.0",{"looper":"^2"})
add("looper","2.0.0",{})
for n,v in index.items():
    open(data+"/index/%s.json"%n,"w").write(json.dumps({"name":n,"versions":v}))
# índice con otro nombre / mal formado
open(data+"/index/wrongname.json","w").write(json.dumps({"name":"other","versions":[]}))
open(data+"/index/badjson.json","w").write("{not json")
open(data+"/index/noversions.json","w").write(json.dumps({"name":"noversions"}))
open(data+"/index/missingfield.json","w").write(json.dumps({"name":"missingfield","versions":[{"version":"1.0.0"}]}))
# paquete cuyo índice apunta a un archivo en http y a un redirect
x=index["gamma"][0]
open(data+"/index/httparch.json","w").write(json.dumps({"name":"httparch","versions":[dict(x,archive="http://localhost:1/x")]}))
open(data+"/index/redirhttp.json","w").write(json.dumps({"name":"redirhttp","versions":[dict(x,archive="https://localhost:%s/redirect-http/x"%port)]}))
open(data+"/index/redirok.json","w").write(json.dumps({"name":"redirok","versions":[dict(x,archive="https://localhost:%s/redirect/gamma-0.3.0.tpkg"%port)]}))
open(data+"/index/missingarch.json","w").write(json.dumps({"name":"missingarch","versions":[dict(x,archive="https://localhost:%s/archive/none.tpkg"%port)]}))
print(json.dumps({n:[v["version"] for v in vs] for n,vs in index.items()}))
