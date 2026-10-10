#!/usr/bin/env python3
"""Servidor del protocolo MySQL para las pruebas de std::mysql (no hay MySQL/MariaDB en el entorno).

Usa `mysql-mimic` (una implementación independiente del protocolo, de terceros) como capa de red y
SQLite de Python como motor SQL. Así los clientes —la VM de Rust (`zett`, crate `mysql`) y el
nativo de Titan— hablan el protocolo de verdad contra el MISMO servidor y se pueden comparar. Es
un sustituto de MySQL, no MySQL: el SQL es el de SQLite y los tipos salen de los valores de Python.
Convenciones para probar otros tipos (por el prefijo del nombre de la columna; el valor viene del texto
que guarde SQLite): dt_ DATETIME ('AAAA-MM-DD HH:MM:SS[.ffffff]'), d_ DATE, t_ TIME ('[-]H:MM:SS[.f]'),
f_ FLOAT, j_ JSON, n_ NEWDECIMAL, b_/s_/l_ TINY/SHORT/LONG con signo, ub_/us_/ul_/u_ y y_ sin signo.
Uso: python3 servidor.py PUERTO [--tls cert.pem key.pem] [--unix RUTA]  (con --unix también atiende
ese socket y `SELECT @@socket` devuelve su ruta; sin él devuelve cadena vacía)
Usuarios: app/secret (mysql_native_password), vacio/(sin clave), clear/clave (clear password),
sha2/sha2pw y sha2v/(sin clave) (caching_sha2_password: la primera vez pide autenticación completa con
clave pública RSA y las siguientes usan el camino rápido), old/oldpw (mysql_old_password).
--handshake sha2 hace que el saludo anuncie caching_sha2_password (por defecto native).
`SELECT @@transport` dice si la conexión llegó por 'unix' o 'tcp'.
"""
import asyncio, os, re, sqlite3, ssl, sys, threading
sys.path.insert(0, os.environ.get("PYLIB", "/home/user/scratch/pylib"))
from cryptography.hazmat.primitives.asymmetric import rsa, padding
from cryptography.hazmat.primitives import hashes, serialization
from mysql_mimic import MysqlServer, ResultColumn
from mysql_mimic.auth import (AbstractClearPasswordAuthPlugin, AuthPlugin, IdentityProvider, NativePasswordAuthPlugin,
                              NoLoginAuthPlugin, User, Success, Forbidden)
from mysql_mimic import utils as _utils, context as _context
from mysql_mimic.stream import MysqlStream
import hashlib
from mysql_mimic.connection import Connection
from mysql_mimic.errors import ErrorCode, MysqlError
from mysql_mimic.session import Session
from mysql_mimic.types import ColumnType, ColumnDefinition

DB = sqlite3.connect(":memory:", check_same_thread=False, isolation_level=None)
LOCK = threading.Lock()
LOCKS = {}
CONEXIONES = [0]
UNIX = [None]

class ClearPlugin(AbstractClearPasswordAuthPlugin):
    name = "mysql_clear_password"
    async def check(self, username, password):
        return username if (username, password) == ("clear", "clave") else None

CLAVES = {"app": "secret", "vacio": "", "clear": "clave", "sha2": "sha2pw", "sha2v": "", "old": "oldpw"}
HANDSHAKE = ["native"]
SHA2_CACHE = set()
FAST = [None]
RSA_KEY = rsa.generate_private_key(public_exponent=65537, key_size=2048)
PUB_PEM = RSA_KEY.public_key().public_bytes(serialization.Encoding.PEM, serialization.PublicFormat.SubjectPublicKeyInfo)

def _sha256(b):
    return hashlib.sha256(b).digest()

def _xor(a, b):
    return bytes(x ^ y for x, y in zip(a, b))

class Sha2Plugin(AuthPlugin):
    """caching_sha2_password: camino rápido tras la primera autenticación completa de cada usuario."""
    name = "caching_sha2_password"
    client_plugin_name = "caching_sha2_password"
    async def auth(self, auth_info=None):
        if auth_info and auth_info.handshake_plugin_name == self.name and auth_info.handshake_auth_data:
            nonce = auth_info.handshake_auth_data.rstrip(b"\x00")
        else:
            nonce = _utils.nonce(20)
            auth_info = yield nonce + b"\x00"
        usuario = auth_info.user.name
        pw = CLAVES[usuario]
        if pw == "":
            yield Success(usuario) if auth_info.data == b"" else Forbidden()
            return
        p = pw.encode()
        esperado = _xor(_sha256(p), _sha256(_sha256(_sha256(p)) + nonce))
        if usuario in SHA2_CACHE and auth_info.data == esperado:
            FAST[0] = _context.connection_id.get()
            yield b"\x03"
            yield Success(usuario)
            return
        auth_info = yield b"\x04"
        dato = auth_info.data
        if dato == b"\x02":
            auth_info = yield PUB_PEM
            claro = RSA_KEY.decrypt(auth_info.data, padding.OAEP(mgf=padding.MGF1(hashes.SHA1()), algorithm=hashes.SHA1(), label=None))
            claro = _xor(claro, (nonce * 10)[:len(claro)]).rstrip(b"\x00")
        else:
            claro = dato.rstrip(b"\x00")
        if claro == p:
            SHA2_CACHE.add(usuario)
            yield Success(usuario)
        else:
            yield Forbidden()

def _hash323(b):
    nr, add, nr2 = 1345345333, 7, 0x12345671
    for x in b:
        if x in (32, 9):
            continue
        nr ^= ((((nr & 63) + add) * x) + (nr << 8)) & 0xFFFFFFFF
        nr2 = (nr2 + (((nr2 << 8) & 0xFFFFFFFF) ^ nr)) & 0xFFFFFFFF
        add = (add + x) & 0xFFFFFFFF
    return nr & 0x7FFFFFFF, nr2 & 0x7FFFFFFF

def _scramble323(nonce, pw):
    h1 = _hash323(pw)
    h2 = _hash323(nonce)
    mx = 0x3FFFFFFF
    s1, s2 = (h1[0] ^ h2[0]) % mx, (h1[1] ^ h2[1]) % mx
    def rnd():
        nonlocal s1, s2
        s1 = (s1 * 3 + s2) % mx
        s2 = (s1 + s2 + 33) % mx
        return s1 / mx
    out = bytearray(int(rnd() * 31) + 64 for _ in range(8))
    extra = int(rnd() * 31)
    return bytes(b ^ extra for b in out)

class OldPlugin(AuthPlugin):
    name = "mysql_old_password"
    client_plugin_name = "mysql_old_password"
    async def auth(self, auth_info=None):
        nonce = bytes(65 + (b % 26) for b in _utils.nonce(8))
        auth_info = yield nonce + b"\x00"
        pw = CLAVES[auth_info.user.name].encode()
        recibido = auth_info.data.rstrip(b"\x00")
        yield Success(auth_info.user.name) if recibido == _scramble323(nonce, pw) else Forbidden()

_auth0 = Connection.authenticate
STREAMS = {}
_leer0 = MysqlStream.read
async def _leer_parcheado(self):
    if FAST[0] is not None and STREAMS.get(FAST[0]) is self:
        FAST[0] = None
        return b""
    return await _leer0(self)
MysqlStream.read = _leer_parcheado
async def _auth_parcheado(self, *a, **kw):
    STREAMS[self.connection_id] = self.stream
    try:
        return await _auth0(self, *a, **kw)
    finally:
        STREAMS.pop(self.connection_id, None)
Connection.authenticate = _auth_parcheado

class Identidad(IdentityProvider):
    def get_plugins(self):
        todos = {"native": NativePasswordAuthPlugin(), "sha2": Sha2Plugin()}
        primero = todos[HANDSHAKE[0]]
        resto = [p for p in todos.values() if p is not primero]
        return [primero] + resto + [ClearPlugin(), OldPlugin(), NoLoginAuthPlugin()]
    async def get_user(self, username):
        if username not in CLAVES:
            return None
        pw = CLAVES[username]
        if username == "clear":
            return User(name=username, auth_plugin="mysql_clear_password")
        if username == "old":
            return User(name=username, auth_plugin="mysql_old_password")
        if username.startswith("sha2"):
            return User(name=username, auth_plugin="caching_sha2_password")
        return User(name=username, auth_string=NativePasswordAuthPlugin.create_auth_string(pw) if pw else None,
                    auth_plugin="mysql_native_password")

# Los paquetes OK de mimic llevan filas afectadas = 0: se inyectan las de la última sentencia.
_ok = Connection.ok
def ok(self, **kw):
    s = self.session
    if "eof" not in kw or not kw["eof"]:
        kw.setdefault("affected_rows", getattr(s, "affected", 0))
        kw.setdefault("last_insert_id", getattr(s, "insert_id", 0))
        s.affected = 0
        s.insert_id = 0
    return _ok(self, **kw)
Connection.ok = ok

import struct
from mysql_mimic import packets as _packets, connection as _connection
from mysql_mimic.types import uint_1, uint_2, uint_4

def _fecha(val):
    m = re.match(r"^(\d+)-(\d+)-(\d+)(?: (\d+):(\d+):(\d+)(?:\.(\d+))?)?$", str(val))
    y, mo, d, h, mi, s, us = [int(x or 0) for x in m.groups()[:6]] + [int((m.group(7) or "0").ljust(6, "0"))]
    return y, mo, d, h, mi, s, us

def _enc_dt(col, val):
    y, mo, d, h, mi, s, us = _fecha(val)
    if us:
        return uint_1(11) + uint_2(y) + uint_1(mo) + uint_1(d) + uint_1(h) + uint_1(mi) + uint_1(s) + uint_4(us)
    if h or mi or s:
        return uint_1(7) + uint_2(y) + uint_1(mo) + uint_1(d) + uint_1(h) + uint_1(mi) + uint_1(s)
    if y or mo or d:
        return uint_1(4) + uint_2(y) + uint_1(mo) + uint_1(d)
    return uint_1(0)

def _enc_d(col, val):
    y, mo, d = _fecha(val)[:3]
    return uint_1(4) + uint_2(y) + uint_1(mo) + uint_1(d) if (y or mo or d) else uint_1(0)

def _enc_t(col, val):
    m = re.match(r"^(-?)(\d+):(\d+):(\d+)(?:\.(\d+))?$", str(val))
    neg = 1 if m.group(1) else 0
    horas, mi, s = int(m.group(2)), int(m.group(3)), int(m.group(4))
    us = int((m.group(5) or "0").ljust(6, "0"))
    dias, h = divmod(horas, 24)
    if us:
        return uint_1(12) + uint_1(neg) + uint_4(dias) + uint_1(h) + uint_1(mi) + uint_1(s) + uint_4(us)
    if dias or h or mi or s:
        return uint_1(8) + uint_1(neg) + uint_4(dias) + uint_1(h) + uint_1(mi) + uint_1(s)
    return uint_1(0)

def _enc_int(fmt):
    return lambda col, val: struct.pack("<" + fmt, int(val))

# prefijo -> (tipo, codificador binario, sin signo)
TIPOS = [("ub_", ColumnType.TINY, _enc_int("B"), True), ("us_", ColumnType.SHORT, _enc_int("H"), True),
         ("ul_", ColumnType.LONG, _enc_int("I"), True), ("u_", ColumnType.LONGLONG, _enc_int("Q"), True),
         ("b_", ColumnType.TINY, _enc_int("b"), False), ("s_", ColumnType.SHORT, _enc_int("h"), False),
         ("l_", ColumnType.LONG, _enc_int("i"), False), ("dt_", ColumnType.DATETIME, _enc_dt, False),
         ("d_", ColumnType.DATE, _enc_d, False), ("t_", ColumnType.TIME, _enc_t, False),
         ("f_", ColumnType.FLOAT, None, False), ("j_", ColumnType.JSON, None, False),
         ("n_", ColumnType.NEWDECIMAL, None, False), ("y_", ColumnType.YEAR, _enc_int("H"), True)]

def _sin_signo(orig):
    def f(*a, **kw):
        n = kw.get("name") or ""
        if any(n.startswith(p) and u for p, _, _, u in TIPOS):
            kw["flags"] = _packets.ColumnDefinition(kw.get("flags", 0)) | _packets.ColumnDefinition.UNSIGNED_FLAG
        return orig(*a, **kw)
    return f
_packets.make_column_definition_41 = _sin_signo(_packets.make_column_definition_41)
_connection.make_column_definition_41 = _sin_signo(_connection.make_column_definition_41)

def columna(nombre, valores):
    for pre, t, enc, _ in TIPOS:
        if nombre.startswith(pre):
            return ResultColumn(nombre, t, binary_encoder=enc)
    for v in valores:
        if v is None:
            continue
        if isinstance(v, bool) or isinstance(v, int):
            return ResultColumn(nombre, ColumnType.LONGLONG)
        if isinstance(v, float):
            return ResultColumn(nombre, ColumnType.DOUBLE)
        if isinstance(v, bytes):
            return ResultColumn(nombre, ColumnType.BLOB)
        return ResultColumn(nombre, ColumnType.VAR_STRING)
    return ResultColumn(nombre, ColumnType.VAR_STRING)

class Sesion(Session):
    def __init__(self):
        super().__init__()
        self.affected = 0
        self.insert_id = 0
        self.mi_bloqueo = set()
    async def init(self, connection):
        await super().init(connection)
        CONEXIONES[0] += 1
    async def close(self):
        await super().close()
        for n in list(self.mi_bloqueo):
            LOCKS.pop(n, None)
    async def handle_query(self, sql, attrs):
        s = sql.strip().rstrip(";").strip()
        up = s.upper()
        m = re.match(r"^SELECT (@@TRANSPORT|@@SOCKET)(?: AS (\w+))?$", up)
        if m:
            alias = (re.search(r" AS (\w+)$", s, re.I) or [None, None])[1]
            if m.group(1) == "@@SOCKET":
                return [(UNIX[0] or "",)], [alias or "@@socket"]
            esunix = self.connection.stream.writer.get_extra_info("peername") in (None, "", b"")
            return [("unix" if esunix else "tcp",)], [alias or "@@transport"]
        if "@@" in s or re.match(r"^(SET|SHOW)\b", up):
            return await super().handle_query(sql, attrs)
        if up in ("START TRANSACTION", "BEGIN"):
            with LOCK:
                DB.execute("BEGIN")
            return None
        if up in ("COMMIT", "ROLLBACK"):
            with LOCK:
                DB.execute(up)
            return None
        m = re.match(r"^SELECT GET_LOCK\('([^']*)',\s*(\d+)\)$", s, re.I)
        if m:
            nombre = m.group(1)
            if LOCKS.get(nombre, self) is not self:
                return [(0,)], ["GET_LOCK"]
            LOCKS[nombre] = self
            self.mi_bloqueo.add(nombre)
            return [(1,)], ["GET_LOCK"]
        m = re.match(r"^SELECT RELEASE_LOCK\('([^']*)'\)$", s, re.I)
        if m:
            LOCKS.pop(m.group(1), None)
            self.mi_bloqueo.discard(m.group(1))
            return [(1,)], ["RELEASE_LOCK"]
        if up.startswith("SELECT SLEEP("):
            await asyncio.sleep(float(re.findall(r"[\d.]+", s)[0]))
            return [(0,)], ["SLEEP"]
        if up.startswith("KILLME"):
            self.connection.stream.writer.transport.abort()
            raise ConnectionResetError()
        try:
            with LOCK:
                cur = DB.execute(s)
                desc = cur.description
                filas = cur.fetchall() if desc else []
                self.affected = cur.rowcount if cur.rowcount > 0 and not desc else 0
                self.insert_id = cur.lastrowid if up.startswith("INSERT") else 0
        except sqlite3.Error as e:
            texto = str(e)
            codigo = ErrorCode.PARSE_ERROR
            if "no such table" in texto:
                codigo = 1146
                texto = "Table 'test.%s' doesn't exist" % texto.rsplit(": ", 1)[-1]
            raise MysqlError(texto, codigo)
        if not desc:
            return None
        nombres = [d[0] for d in desc]
        cols = [columna(n, [f[i] for f in filas]) for i, n in enumerate(nombres)]
        return filas, cols

async def principal():
    port = int(sys.argv[1])
    kw = {}
    ctx = None
    unix = None
    args = sys.argv[2:]
    while args:
        a = args.pop(0)
        if a == "--tls":
            ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            ctx.load_cert_chain(args.pop(0), args.pop(0))
        elif a == "--unix":
            unix = args.pop(0)
        elif a == "--handshake":
            HANDSHAKE[0] = args.pop(0)
    UNIX[0] = unix
    servers = [MysqlServer(session_factory=Sesion, identity_provider=Identidad(), ssl=ctx, port=port, host="127.0.0.1")]
    if unix:
        if os.path.exists(unix):
            os.unlink(unix)
        su = MysqlServer(session_factory=Sesion, identity_provider=Identidad(), ssl=ctx)
        await su.start_unix_server(path=unix)
        servers.append(su)
    await asyncio.gather(*(sv.serve_forever() for sv in servers))

asyncio.run(principal())
