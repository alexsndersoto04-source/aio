import sqlite3, sys
for f in sys.argv[1:]:
    c = sqlite3.connect(f)
    print(f, c.execute("pragma integrity_check").fetchall(), c.execute("select count(*), sum(a), sum(length(b)) from t").fetchall(), c.execute("select count(*) from t indexed by i1").fetchall(), c.execute("select count(*) from t indexed by i2").fetchall(), c.execute("pragma freelist_count").fetchall())
