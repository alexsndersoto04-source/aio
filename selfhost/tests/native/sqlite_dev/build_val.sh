#!/bin/bash
D=$(dirname "$0"); N=$D/../../../native
cat $N/std_sqlite_val.titan $D/t_val_main.titan > /tmp/t_val.titan
