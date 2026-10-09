#!/bin/bash
D=$(dirname "$0"); N=$D/../../../native
cat $N/std_sqlite_val.titan $N/std_sqlite_pg.titan $N/std_sqlite_bt.titan $D/${1:-t_bt_main}.titan > /tmp/t_bt.titan
