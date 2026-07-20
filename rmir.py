import os
import zlib
import base64

# He consolidado la cadena para evitar errores de corte.
# Si el código original falla, es porque la cadena base64 está truncada.
# Asegúrate de que este bloque contenga EXACTAMENTE el código base64 completo.
data_encoded = """
eNrtXety2ziy/u+ngJ1aj7RHUSTb8aTkcbYcx5lky46nnMxs
tlwqFiVCNncoUsOLL5u4ah/iPOF5ktPdAEmABClKspPZGbtS
ikgCjUbj6xsu1LNn6+yjG9s+O3l3xv7vP//LTlznqcevuMfg
xrUbX7KRHbljNvKC8a9Rh125YZzYHgv5hRvFPIRbtu+wceDH
YeCxiRdcd9fWZsmITQOH2eF0d2cvu+TeJL+4ebFr4cO1JOIs
ip3BYBx4Hh/HbuBHg8FbO7o8sWd79DhGHi07igeDz69c3w5v
T2fMjthBFMPl6azDXiF/HXZ0Mws77EM8jTvsZ62cvLpTCV66
IRB864ZvEp/a7TC4+CkMLkJ7CiXXnpw7PHSveOs1HyUXHXbo
BT7vsJ/sMHZt7+i39pC6E8VhMo5BduFJ4CQeZ5/XGPzho4mk
HA3YL3z8w0ne1svO2t0SLaTVlTZ8e8oH0OnQ9S862d2ZDZ2Q
zYpnL/OHYjgznkh6ymMOo3k7YEnk/psvxyZRVHh0nYxcdssX
xTXpvIObC0mG+8mUyYqyvZPgCoaAOQAX2SaLwvEAS53OeIh4
vRNsEHgKRYPZgElQeZeRWqvDQv1GSiZFWpmQfIIXVMNU+ziw
nULVWRzqDQeTScThubu9lVb7EAchdrOubGXHD23PS9s8neEQ
/EBNv+ywMTziOZ5Ahy/y4ZFkXmZM3EbjalIoAVkCZVBL6RVc
jS/fTYAUGBNH71J8yX2LMJuJiHsR126lhP6eTGctutMWN854
3JKMKe3Kh4fBdApob4netuuQdxjMbhX8gbHRMJh1VKLwjNuO
aOMfoRtL3EPrvmzYCyJ58+jGjcW3k6k9Wwb86eimLV9oAng3
nbZQQ6CnjmvH6e2ffYdPllK1lJBs7x3Iz93dkWTfeIEdtybZ
9asg8Foj+GinuEVJ5wLHe+9dbwXJSy0mSgcOwOVDMuqwk8Tr
sNfuFXwL4N4B4ugUnMOnIEQCHfaew8cxOIofY/yf4xe+AhuZ
FRBd4lDxfQCkX7kx/X8G0saPkwQuXvNQCp+8hM/AcfIQPZIV
B9bUDVvonNhm7o/a7OnLkovxeMymSZy7GbaPCjYY+Py61d6j
MpMgpOdgcNkmUO3mhQUR/HMnROtDMOUtaN3CMm0gJthKa7TE
7bwa0U/JdWdJdJnX3stK3a3ln0oPFK7vUBKK90hcD4ZANuTz
m9iCgEPzIDUebJyEIei0bjCEuAJQUSQFFWWA8UNq56TREsRD
bv9qxWCweMmqiSYg3nH9hFeVgd6405lX7guMNA0NDuYHiIcU
WRYu9Z73OtqDtPdXfLx+nrtbcrO9Ttm1SkCwu6FOpyCqQisG
cUlCBW40cb1HZdFbKUpLL5LCQxEQNtraRGRHIBWSljDyuXwQ
rlAKMIpFuqmoctRpt9n/7LN+/gzuGBolKTRo1nXSVsU4dD3u
t9qFluUj0glthOqGRyHiOgUOIwjHx5doHgp8dvLoStVN4kIb
YODadfYKZPnUjTVaPnpyGVGV6IlenZdpD7tqr0S38U672J4b
iQoWZA9TiOVj7rQ2M3Gjo6gwS56N/OwvxgdWahUt1tQGQfJo
nUh20r4OBhgpWG32Jb9DwYR+SwlUul0YsRzGFJMUjaMN90pI
B2lkJl+zrcLipzE+SSQPXbLMQTH+o8C5BZlg3S5+79oRwH3S
av9tT3MQI2mF9hWTpHmJtKBIGwpuJPMj9JQcCbUoC+ddFjfE
6NP3LiYn3TE60FZbQXeuvZKzbqr1SqH0UW6HcHB5GBtod5Cc
""".replace("\n", "").replace(" ", "")

try:
    decoded_data = base64.b64decode(data_encoded)
    decompressed_data = zlib.decompress(decoded_data)
    
    d = os.path.join("crates", "titan_mir", "src")
    os.makedirs(d, exist_ok=True)
    path = os.path.join(d, "lib.rs")
    
    with open(path, "wb") as f:
        f.write(decompressed_data)
        
    print(f"[OK] Archivo restaurado: {len(decompressed_data)} bytes.")
except Exception as e:
    print(f"[ERROR] Falló la reconstrucción: {e}")
