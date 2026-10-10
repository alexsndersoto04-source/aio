#!/bin/bash
# Compone el programa de prueba del motor regex (solo desarrollo; usa el oráculo `zett`).
D=$(dirname "$0"); N=$D/../../../native
cat $N/std_tokenize_tab.titan $N/std_tokenize_uni.titan $N/std_tokenize_rx.titan $D/rx_cases.titan $D/rx_main.titan > ${1:-/tmp/rx_all.titan}
