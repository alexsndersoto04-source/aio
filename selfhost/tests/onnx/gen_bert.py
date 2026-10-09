#!/usr/bin/env python3
"""Genera modelos BERT reales (transformers + torch.onnx) y su referencia de onnxruntime.

Necesita `torch`, `transformers`, `onnx`, `onnxruntime`, `numpy` (y `onnxscript` para el exportador
dynamo). Uso: gen_bert.py <directorio>. Escribe <nombre>.onnx y <nombre>.json
({"inputs": [{"shape", "data"}...], "ref": [{"shape", "values"}...]}).
Los pesos son aleatorios (semilla fija): el grafo es el de un BERT de HuggingFace de verdad,
con ids reales de un vocabulario de 1000 palabras y una máscara con relleno.
"""
import json
import sys
import warnings

import numpy as np
import onnxruntime as ort
import torch
from transformers import BertConfig, BertModel

warnings.filterwarnings("ignore")
OUT = sys.argv[1]


class Wrap(torch.nn.Module):
    def __init__(self, m):
        super().__init__()
        self.m = m

    def forward(self, *a):
        o = self.m(*a)
        return o.last_hidden_state, o.pooler_output


def make(name, hidden, layers, heads, batch, seq, three, legacy, dynamic):
    torch.manual_seed(7)
    cfg = BertConfig(vocab_size=1000, hidden_size=hidden, num_hidden_layers=layers,
                     num_attention_heads=heads, intermediate_size=hidden * 2,
                     max_position_embeddings=64, attn_implementation="eager")
    model = Wrap(BertModel(cfg).eval())
    ids = torch.randint(0, 1000, (batch, seq))
    mask = torch.ones(batch, seq, dtype=torch.long)
    mask[-1, seq - 3:] = 0
    types = torch.zeros(batch, seq, dtype=torch.long)
    types[:, seq // 2:] = 1
    args = (ids, mask, types) if three else (ids, mask)
    names = ["input_ids", "attention_mask", "token_type_ids"][:len(args)]
    path = f"{OUT}/{name}.onnx"
    if legacy:
        axes = {n: {0: "batch", 1: "seq"} for n in names} if dynamic else None
        torch.onnx.export(model, args, path, input_names=names,
                          output_names=["last_hidden_state", "pooler_output"],
                          opset_version=17, dynamo=False, dynamic_axes=axes)
    else:
        torch.onnx.export(model, args, path, input_names=names,
                          output_names=["last_hidden_state", "pooler_output"],
                          dynamo=True, external_data=False)
    sess = ort.InferenceSession(path, providers=["CPUExecutionProvider"])
    feeds = {n: a.numpy() for n, a in zip(names, args)}
    ref = sess.run(None, feeds)
    doc = {
        "inputs": [{"shape": list(a.shape), "data": a.numpy().reshape(-1).tolist()} for a in args],
        "ref": [{"shape": list(r.shape), "values": [float(v) for v in r.reshape(-1)]} for r in ref],
    }
    json.dump(doc, open(f"{OUT}/{name}.json", "w"))
    print(f"{name}: {len(args)} entradas, salidas {[list(r.shape) for r in ref]}")


make("bert_legacy", 64, 2, 2, 2, 8, False, True, False)
make("bert_legacy3", 64, 2, 2, 2, 8, True, True, True)
make("bert_dynamo", 64, 2, 2, 2, 8, False, False, False)
make("bert_mid", 128, 3, 4, 2, 12, False, True, True)
