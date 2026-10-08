#!/usr/bin/env python3
# Genera selfhost/native/std_tz_data.titan: la base de datos de zonas horarias
# de IANA tal como la usa chrono-tz 0.10.4 (tzdb 2025b, solo los archivos
# principales, sin 'backzone').
#
# Entradas (en este directorio, de dominio público):
#   tzdata-2026b.zi     copia de /usr/share/zoneinfo/tzdata.zi de Debian
#                       (tzdb 2026b compilada con backzone)
#   backward_2025b.txt  las líneas Link del archivo 'backward' de tzdb 2025b
#
# Pasos:
#   1. Se leen reglas, zonas y enlaces del .zi.
#   2. Las zonas de 'backzone' que en tzdb 2025b son enlaces se sustituyen
#      por esos enlaces (backward_2025b.txt).
#   3. Se deshacen los tres cambios de datos entre 2025b y 2026b (ver NEWS de
#      tzdb): 2025c Tijuana (1953 y 1961-1975), 2026a Moldavia (horas de la UE
#      desde 2022), 2026b Columbia Británica (-07 fijo desde 2026-11-01).
#   4. Se escribe en un formato numérico sencillo que el runtime de Titan lee.
#
# Además, este archivo contiene una copia en Python del algoritmo de
# parse-zoneinfo 0.5 (el que generó las tablas de chrono-tz) para comprobar
# los datos contra la VM (ver verify_tz.py).
import os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
MONTHS = ['ja', 'f', 'mar', 'ap', 'may', 'jun', 'jul', 'au', 's', 'o', 'n', 'd']
MONTH_FULL = ['january', 'february', 'march', 'april', 'may', 'june', 'july',
              'august', 'september', 'october', 'november', 'december']
WDAYS = ['sunday', 'monday', 'tuesday', 'wednesday', 'thursday', 'friday', 'saturday']
MAXY = 99999


def month_of(s):
    s = s.lower()
    hits = [i for i, m in enumerate(MONTH_FULL) if m.startswith(s)]
    assert len(hits) == 1, s
    return hits[0] + 1


def wday_of(s):
    s = s.lower()
    hits = [i for i, w in enumerate(WDAYS) if w.startswith(s)]
    assert len(hits) == 1, s
    return hits[0]


def secs(s):
    # Mismo resultado que TimeSpec::as_seconds de parse-zoneinfo.
    if s == '-':
        return 0
    neg = s.startswith('-')
    parts = s.lstrip('-').split(':')
    v = int(parts[0]) * 3600
    if len(parts) > 1:
        v += int(parts[1]) * 60
    if len(parts) > 2:
        v += int(parts[2])
    assert len(parts) <= 3
    return -v if neg else v


def timetype(s):
    # 'w' pared, 's' estándar, 'u' UTC (también 'g', 'z')
    if s and s[-1] in 'wsugz':
        t = s[-1]
        return s[:-1], {'w': 'w', 's': 's', 'u': 'u', 'g': 'u', 'z': 'u'}[t]
    return s, 'w'


def dayspec(s):
    # (tipo, día de la semana, número): 0 ordinal, 1 último, 2 <=, 3 >=
    if s.isdigit():
        return (0, 0, int(s))
    if s.startswith('last'):
        return (1, wday_of(s[4:]), 0)
    for op, k in (('>=', 3), ('<=', 2)):
        if op in s:
            a, b = s.split(op)
            return (k, wday_of(a), int(b))
    raise ValueError(s)


def year_of(s):
    s2 = s.lower()
    if 'maximum'.startswith(s2) and len(s2) >= 2:
        return MAXY
    return int(s)


# ------------------------------------------------------------ lectura


def read_zi(path):
    rules, zones, links, order = {}, {}, {}, []
    cur = None
    for raw in open(path, encoding='utf-8'):
        line = raw.split('#', 1)[0].rstrip('\n')
        if not line.strip():
            continue
        f = line.split()
        if f[0] == 'R':
            _, name, fr, to, typ, mon, on, at, save, let = f
            assert typ == '-'
            fy = year_of(fr)
            ty = fy if to.lower() in ('o', 'only') else year_of(to)
            t, tt = timetype(at)
            rules.setdefault(name, []).append(
                dict(fy=fy, ty=ty, mon=month_of(mon), day=dayspec(on), time=secs(t),
                     tt=tt, save=secs(save), letters=None if let == '-' else let))
            cur = None
        elif f[0] == 'Z':
            cur = f[1]
            assert cur not in zones
            zones[cur] = [zone_line(f[2:])]
            order.append(cur)
        elif f[0] == 'L':
            links[f[2]] = f[1]
            cur = None
        else:
            assert cur is not None, line
            zones[cur].append(zone_line(f))
    return rules, zones, links, order


def zone_line(f):
    off, rules, fmt = f[0], f[1], f[2]
    if rules == '-':
        saving = ('none', 0)
    elif all(c == '-' or c == '_' or c.isalpha() for c in rules):
        saving = ('rules', rules)
    else:
        saving = ('fixed', secs(rules))
    until = None
    u = f[3:]
    if u:
        y = int(u[0])
        m = month_of(u[1]) if len(u) > 1 else 1
        d = dayspec(u[2]) if len(u) > 2 else (0, 0, 1)
        if len(u) > 3:
            t, tt = timetype(u[3])
            tsec = secs(t)
        else:
            tsec, tt = 0, 'w'
        # kind: 0 año, 1 mes, 2 día, 3 hora (como ChangeTime)
        until = (min(len(u), 4) - 1, y, m, d, tsec, tt)
    return dict(off=secs(off), saving=saving, fmt=fmt, until=until)


def parse_zl(text):
    return zone_line(text.split())


def build_2025b():
    rules, zones, links, order = read_zi(os.path.join(HERE, 'tzdata-2026b.zi'))
    back = {}
    for l in open(os.path.join(HERE, 'backward_2025b.txt'), encoding='utf-8'):
        if l.startswith('Link'):
            _, target, name = l.split()
            back[name] = target
    # 2. backzone -> enlaces de 2025b
    for name, target in back.items():
        if name in zones:
            del zones[name]
            order.remove(name)
        links[name] = target
    # 'Factory' está en el archivo 'factory', que chrono-tz no incluye.
    if 'Factory' in zones:
        del zones['Factory']
        order.remove('Factory')
    # 3a. 2025c: Tijuana.  2025b tenía, tras "PDT 1952 Sep 28 2:00":
    #     -8:00 - PST 1954 / -8:00 CA P%sT 1961 / -8:00 - PST 1976 / -8:00 US P%sT 1996
    tj = zones['America/Tijuana']
    i = [k for k, z in enumerate(tj) if z['saving'] == ('rules', 'CA')]
    assert len(i) == 1 and tj[i[0]]['until'][1] == 1967 and tj[i[0] + 1]['saving'] == ('rules', 'u')
    i = i[0]
    tj[i:i + 1] = [parse_zl('-8 - PST 1954'), parse_zl('-8 CA P%sT 1961'),
                   parse_zl('-8 - PST 1976')]
    # 3b. 2026a: Moldavia.  2025b: Rule Moldova 1997 max; la zona acababa en
    #     "2:00 Moldova EE%sT".
    md = rules['MD']
    assert [r['ty'] for r in md] == [2021, 2021]
    for r in md:
        r['ty'] = MAXY
    ch = zones['Europe/Chisinau']
    assert ch[-2]['saving'] == ('rules', 'MD') and ch[-2]['until'][1] == 2022
    assert ch[-1]['saving'] == ('rules', 'E')
    ch[-2:] = [parse_zl('2 MD EE%sT')]
    # 3c. 2026b: Columbia Británica.  2025b: "-8:00 Canada P%sT" sin fin.
    va = zones['America/Vancouver']
    assert va[-3]['saving'] == ('rules', 'C') and va[-3]['until'][1] == 2026
    va[-3:] = [parse_zl('-8 C P%sT')]
    for n, t in links.items():
        assert t in zones, (n, t)
    return rules, zones, links, order


# ------------------------------------------------------------ parse-zoneinfo


def is_leap(y):
    return y & 3 == 0 and (y % 25 != 0 or y & 15 == 0)


def mlen(m, leap):
    return [31, 29 if leap else 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1]


def weekday(y, m, d):
    T = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4]
    if m < 3:
        y -= 1
    # i64 de Rust: / y % truncan hacia cero
    def tdiv(a, b):
        q = abs(a) // abs(b)
        return q if (a >= 0) == (b >= 0) else -q
    v = y + tdiv(y, 4) - tdiv(y, 100) + tdiv(y, 400) + T[m - 1] + d
    r = abs(v) % 7
    if v < 0:
        r = -r
    assert r >= 0, 'why is negative modulus designed so?'
    return r


def concrete_day(day, y, m):
    kind, w, n = day
    leap = is_leap(y)
    length = mlen(m, leap)
    prev_length = mlen(m - 1, leap) if m > 1 else 0
    if kind == 0:
        return m, n
    if kind == 1:
        for d in range(length, 0, -1):
            if weekday(y, m, d) == w:
                return m, d
        raise AssertionError
    if kind == 2:
        for d in range(n, -8, -1):
            if d >= 1 and weekday(y, m, d) == w:
                return m, d
            if d < 1:
                assert m > 1
                if weekday(y, m - 1, prev_length + d) == w:
                    return m - 1, prev_length + d
        raise AssertionError
    for d in range(n, n + 8):
        if d <= length and weekday(y, m, d) == w:
            return m, d
        if d > length:
            assert m < 12
            if weekday(y, m + 1, d - length) == w:
                return m + 1, d - length
    raise AssertionError


def days_before_year(y):
    s = 0
    if y >= 1970:
        for k in range(1970, y):
            s += 366 if is_leap(k) else 365
    else:
        for k in range(y, 1970):
            s -= 366 if is_leap(k) else 365
    return s


CUM = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334]


def ttime(y, m, d, secs_of_day):
    leap_extra = 1 if (is_leap(y) and m > 2) else 0
    return (days_before_year(y) + CUM[m - 1] + leap_extra + d - 1) * 86400 + secs_of_day


def until_ts(u, utc, dst):
    kind, y, m, d, t, tt = u
    if kind == 0:
        return ttime(y, 1, 1, 0) - (utc + dst)
    if kind == 1:
        return ttime(y, m, 1, 0) - (utc + dst)
    mm, dd = concrete_day(d, y, m)
    if kind == 2:
        return ttime(y, mm, dd, 0) - (utc + dst)
    return ttime(y, mm, dd, t) - {'u': 0, 's': utc, 'w': utc + dst}[tt]


def rule_abs(r, y, utc, dst):
    off = {'u': 0, 's': utc, 'w': utc + dst}[r['tt']]
    mm, dd = concrete_day(r['day'], y, r['mon'])
    return ttime(y, mm, dd, 0) + r['time'] - off


def fmt(f, utc, dst, letters):
    if '/' in f:
        p = f.index('/')
        return f[:p] if dst == 0 else f[p + 1:]
    if '%s' in f:
        return f.replace('%s', letters or '')
    if f == '%z':
        o = utc + dst
        sign = '-' if o < 0 else '+'
        o = abs(o)
        assert o % 60 == 0
        h, mi = o // 3600, (o // 60) % 60
        return sign + '%02d' % h + ('%02d' % mi if mi else '')
    return f


def timespans(rules, zoneset):
    first = None
    rest = []
    start_time = None
    for i, z in enumerate(zoneset):
        dst = 0
        use_until = i != len(zoneset) - 1
        utc = z['off']
        ins = i > 0
        szid = None
        s_utc, s_dst = z['off'], 0
        kind, val = z['saving']
        if kind in ('none', 'fixed'):
            amount = 0 if kind == 'none' else val
            dst = amount
            szid = fmt(z['fmt'], utc, dst, None)
            if ins:
                rest.append((start_time, (z['off'], dst, szid)))
                ins = False
            else:
                first = (utc, dst, szid)
        else:
            rs = rules[val]
            for year in range(1800, 2100):
                if use_until and year > z['until'][1]:
                    break
                act = [r for r in rs if r['fy'] <= year <= r['ty']]
                while True:
                    until = until_ts(z['until'], utc, dst) if use_until else None
                    if not act:
                        break
                    best, bt = 0, rule_abs(act[0], year, utc, dst)
                    for k in range(1, len(act)):
                        t = rule_abs(act[k], year, utc, dst)
                        if t < bt:
                            best, bt = k, t
                    r = act.pop(best)
                    if use_until and bt >= until:
                        break
                    dst = r['save']
                    if ins and bt == start_time:
                        ins = False
                    if ins:
                        if bt < start_time:
                            s_utc, s_dst = z['off'], dst
                            szid = fmt(z['fmt'], utc, dst, r['letters'])
                            continue
                        if szid is None and s_utc + s_dst == z['off'] + dst:
                            szid = fmt(z['fmt'], utc, dst, r['letters'])
                    rest.append((bt, (z['off'], r['save'], fmt(z['fmt'], z['off'], r['save'], r['letters']))))
        if ins and szid is not None:
            rest.append((start_time, (s_utc, s_dst, szid)))
        if use_until:
            start_time = until_ts(z['until'], utc, dst)
    rest.sort(key=lambda e: e[0])
    if first is None:
        first = next(e[1] for e in rest if e[1][1] == 0)
    # optimise
    fi = ti = 0
    while fi < len(rest):
        if ti > 1:
            a = rest[fi][0]
            b = rest[ti - 1][0]
            if a + sum(rest[ti - 1][1][:2]) <= b + sum(rest[ti - 2][1][:2]):
                rest[ti - 1] = (rest[ti - 1][0], rest[fi][1])
                fi += 1
                continue
        if ti == 0 or rest[ti - 1][1] != rest[fi][1]:
            rest[ti] = rest[fi]
            ti += 1
        fi += 1
    del rest[ti:]
    if rest and first == rest[0][1]:
        del rest[0]
    return first, rest


def offset_at(spans, ts):
    first, rest = spans
    # utc_span: [rest[i-1].0, rest[i].0)
    lo, hi = 0, len(rest)
    # índice = número de transiciones <= ts
    import bisect
    k = bisect.bisect_right([t for t, _ in rest], ts)
    s = first if k == 0 else rest[k - 1][1]
    return s[0] + s[1]


# ------------------------------------------------------------ salida


def emit(rules, zones, links, order, path):
    # Formato (una entrada por línea, campos separados por un espacio):
    #   R nombre desde hasta mes tipo_día día_semana número segundos tipo ahorro letras
    #   Z nombre                         (empieza una zona)
    #   C desplazamiento tipo_ahorro valor formato [clase año mes tipo_día
    #     día_semana número segundos tipo]   (una línea de la zona)
    #   L destino nombre
    # 'hasta' = 99999 para "max"; letras '-' = ninguna; tipo_ahorro n/f/r.
    out = []
    for name, rs in rules.items():
        for r in rs:
            k, w, n = r['day']
            out.append('R %s %d %d %d %d %d %d %d %s %d %s' % (
                name, r['fy'], r['ty'], r['mon'], k, w, n, r['time'], r['tt'], r['save'],
                r['letters'] if r['letters'] is not None else '-'))
    for name in order:
        out.append('Z ' + name)
        for z in zones[name]:
            kind, val = z['saving']
            s = 'C %d %s %s %s' % (z['off'], kind[0], val if kind == 'rules' else str(val), z['fmt'])
            if z['until']:
                c, y, m, d, t, tt = z['until']
                s += ' %d %d %d %d %d %d %d %s' % (c, y, m, d[0], d[1], d[2], t, tt)
            out.append(s)
    for name in sorted(links):
        out.append('L %s %s' % (links[name], name))
    text = '\n'.join(out) + '\n'
    assert '"' not in text and '\\' not in text and '{' not in text and '}' not in text
    # Trozos de <= 4000 caracteres, cortados en fin de línea.
    chunks, cur = [], ''
    for line in text.splitlines(True):
        if len(cur) + len(line) > 4000:
            chunks.append(cur)
            cur = ''
        cur += line
    chunks.append(cur)
    with open(path, 'w', encoding='utf-8') as f:
        f.write('// GENERADO por tz/gen_tz.py: base de datos de zonas horarias de IANA\n')
        f.write('// (tzdb 2025b sin backzone, la de chrono-tz 0.10.4). No editar a mano.\n\n')
        f.write('fn tz_data_parts() -> array {\n    [\n')
        for c in chunks:
            f.write('        "' + c.replace('\n', '\\n') + '",\n')
        f.write('    ]\n}\n')
    return text


if __name__ == '__main__':
    rules, zones, links, order = build_2025b()
    emit(rules, zones, links, order, os.path.join(HERE, '..', 'std_tz_data.titan'))
    print('zonas', len(zones), 'enlaces', len(links), 'reglas', sum(len(v) for v in rules.values()))
