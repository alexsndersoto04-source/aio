#!/bin/bash
# Compone el programa de comparación del motor completo (solo desarrollo; usa el oráculo `zett`).
D=$(dirname "$0"); N=$D/../../../native
cat $N/std_tokenize_tab.titan $N/std_tokenize_uni.titan $N/std_tokenize_rx.titan $N/std_tokenize_eng.titan $N/std_tokenize_mod.titan $N/std_tokenize_dec.titan $N/std_tokenize_unigram.titan $N/std_tokenize_load.titan $D/cmp_main.titan > ${1:-/tmp/cmp_all.titan}
