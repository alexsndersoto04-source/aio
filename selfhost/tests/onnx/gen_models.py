#!/usr/bin/env python3
"""Genera los modelos ONNX de prueba de std::onnx y las salidas de referencia.

Necesita `numpy`, `onnx` y `onnxruntime` (p. ej. en un venv:
`python3 -m venv /tmp/oxv && /tmp/oxv/bin/pip install numpy onnx onnxruntime`).

Para cada caso escribe en <dir>:
  <nombre>.onnx   el modelo
  <nombre>.json   {"shape": [...], "data": [...], "ref": {"shape": [...], "values": [...]},
                   "in_shapes": [...], "out_shapes": [...]}
La referencia sale de onnxruntime (el oráculo independiente); el comparador
(compare.py) mira además lo que da tract a través de `zett`.
"""
import json
import sys

import numpy as np
import onnx
import onnxruntime as ort
from onnx import TensorProto, helper, numpy_helper

OUT = sys.argv[1] if len(sys.argv) > 1 else "."
rng = np.random.default_rng(20261008)
F = TensorProto.FLOAT
I64 = TensorProto.INT64


def rnd(*shape, scale=1.0):
    return (rng.standard_normal(shape) * scale).astype(np.float32)


def init(name, arr):
    return numpy_helper.from_array(np.asarray(arr), name)


def vi(name, shape, elem=F):
    return helper.make_tensor_value_info(name, elem, shape)


def case(name, nodes, in_shape, out_name="y", inits=(), opset=13, in_dims=None, x=None,
         ir=8, elem_in=F):
    """in_dims: forma declarada de la entrada (None dentro = dinámica)."""
    def build(out_shape):
        graph = helper.make_graph(
            nodes, name,
            [vi("x", in_dims if in_dims is not None else in_shape, elem_in)],
            [helper.make_tensor_value_info(out_name, F, out_shape)],
            initializer=list(inits))
        model = helper.make_model(graph, opset_imports=[helper.make_opsetid("", opset)])
        model.ir_version = ir
        return model

    # Primera pasada sin comprobar para conocer la forma de salida; la segunda
    # declara esa forma y pasa el comprobador de onnx.
    onnx.save(build(None), f"{OUT}/{name}.onnx")
    if x is None:
        x = rnd(*in_shape)
    sess = ort.InferenceSession(f"{OUT}/{name}.onnx", providers=["CPUExecutionProvider"])
    y = sess.run(None, {"x": x})[0]
    model = build(list(y.shape))
    onnx.checker.check_model(model)
    onnx.save(model, f"{OUT}/{name}.onnx")
    sess = ort.InferenceSession(f"{OUT}/{name}.onnx", providers=["CPUExecutionProvider"])
    y = sess.run(None, {"x": x})[0]
    doc = {
        "shape": list(x.shape),
        "ids": elem_in == I64,
        "data": [int(v) if elem_in == I64 else float(v) for v in x.reshape(-1)],
        "ref": {"shape": list(y.shape), "values": [float(v) for v in y.reshape(-1)]},
    }
    with open(f"{OUT}/{name}.json", "w") as f:
        json.dump(doc, f)
    print(f"{name}: in {list(x.shape)} -> out {list(y.shape)}")


N = helper.make_node

# --- MLP: Gemm + Relu + Gemm + Softmax
case("mlp", [
    N("Gemm", ["x", "w1", "b1"], ["h1"]),
    N("Relu", ["h1"], ["h2"]),
    N("Gemm", ["h2", "w2", "b2"], ["h3"], transB=1, alpha=0.5),
    N("Softmax", ["h3"], ["y"], axis=-1),
], (3, 8), inits=[init("w1", rnd(8, 16, scale=0.5)), init("b1", rnd(16, scale=0.1)),
                  init("w2", rnd(5, 16, scale=0.5)), init("b2", rnd(5, scale=0.1))],
     in_dims=["N", 8])

# --- MLP con MatMul + Add (lo que exporta PyTorch sin Gemm) + Sigmoid + Tanh
case("mlp_matmul", [
    N("MatMul", ["x", "w1"], ["m1"]),
    N("Add", ["m1", "b1"], ["a1"]),
    N("Sigmoid", ["a1"], ["s1"]),
    N("MatMul", ["s1", "w2"], ["m2"]),
    N("Tanh", ["m2"], ["y"]),
], (2, 6), inits=[init("w1", rnd(6, 10)), init("b1", rnd(10)), init("w2", rnd(10, 4))])

# --- CNN completa: Conv+BN+Relu+MaxPool+Conv(strided)+GAP+Flatten+Gemm
case("cnn", [
    N("Conv", ["x", "c1w", "c1b"], ["c1"], kernel_shape=[3, 3], pads=[1, 1, 1, 1]),
    N("BatchNormalization", ["c1", "bns", "bnb", "bnm", "bnv"], ["bn"], epsilon=1e-3),
    N("Relu", ["bn"], ["r1"]),
    N("MaxPool", ["r1"], ["p1"], kernel_shape=[2, 2], strides=[2, 2]),
    N("Conv", ["p1", "c2w"], ["c2"], kernel_shape=[3, 3], strides=[2, 2]),
    N("LeakyRelu", ["c2"], ["r2"], alpha=0.1),
    N("GlobalAveragePool", ["r2"], ["g"]),
    N("Flatten", ["g"], ["f"]),
    N("Gemm", ["f", "fw", "fb"], ["y"]),
], (2, 3, 12, 12), inits=[
    init("c1w", rnd(4, 3, 3, 3, scale=0.4)), init("c1b", rnd(4, scale=0.1)),
    init("bns", np.abs(rnd(4)) + 0.5), init("bnb", rnd(4, scale=0.1)),
    init("bnm", rnd(4, scale=0.1)), init("bnv", np.abs(rnd(4)) + 0.5),
    init("c2w", rnd(6, 4, 3, 3, scale=0.3)),
    init("fw", rnd(6, 3)), init("fb", rnd(3))])

# --- Conv con grupos, dilatación y auto_pad
case("conv_group", [
    N("Conv", ["x", "w"], ["y"], kernel_shape=[3, 3], group=2, dilations=[2, 2],
      pads=[2, 2, 2, 2], strides=[1, 2]),
], (1, 4, 9, 9), inits=[init("w", rnd(6, 2, 3, 3, scale=0.4))])
case("conv_same", [
    N("Conv", ["x", "w", "b"], ["y"], kernel_shape=[3, 3], auto_pad="SAME_UPPER", strides=[2, 2]),
], (1, 2, 7, 8), inits=[init("w", rnd(3, 2, 3, 3)), init("b", rnd(3))])
case("conv1d", [
    N("Conv", ["x", "w", "b"], ["y"], kernel_shape=[3], pads=[1, 2], strides=[2]),
], (2, 3, 11), inits=[init("w", rnd(4, 3, 3)), init("b", rnd(4))])

# --- Pooling
case("avgpool", [
    N("AveragePool", ["x"], ["y"], kernel_shape=[3, 3], strides=[2, 2], pads=[1, 1, 1, 1],
      count_include_pad=1),
], (1, 2, 8, 8))
case("avgpool_ceil", [
    N("AveragePool", ["x"], ["y"], kernel_shape=[2, 2], strides=[2, 2], ceil_mode=1),
], (1, 1, 7, 7))
case("maxpool_pad", [
    N("MaxPool", ["x"], ["y"], kernel_shape=[3, 3], strides=[2, 2], pads=[1, 1, 1, 1]),
], (1, 2, 9, 9))
case("gmaxpool", [N("GlobalMaxPool", ["x"], ["y"])], (2, 3, 5, 4))

# --- Elementwise + broadcasting
case("elem", [
    N("Mul", ["x", "k"], ["a"]),
    N("Add", ["a", "b"], ["c"]),
    N("Sub", ["c", "x"], ["d"]),
    N("Div", ["d", "two"], ["e"]),
    N("Clip", ["e", "lo", "hi"], ["f"]),
    N("Abs", ["f"], ["g"]),
    N("Neg", ["g"], ["h"]),
    N("Exp", ["h"], ["y"]),
], (2, 3, 4), inits=[init("k", rnd(3, 1)), init("b", rnd(4)),
                      init("two", np.float32(2.0)),
                      init("lo", np.float32(-0.5)), init("hi", np.float32(0.7))])
case("sqrt_log", [
    N("Abs", ["x"], ["a"]),
    N("Add", ["a", "one"], ["b"]),
    N("Sqrt", ["b"], ["c"]),
    N("Log", ["c"], ["y"]),
], (3, 5), inits=[init("one", np.float32(1.0))])

# --- Matmul por lotes
case("matmul_batch", [N("MatMul", ["x", "w"], ["y"])], (2, 3, 4), inits=[init("w", rnd(4, 5))])
case("matmul_batch2", [N("MatMul", ["x", "w"], ["y"])], (2, 1, 3, 4),
     inits=[init("w", rnd(3, 4, 2))])

# --- Formas
case("shapes", [
    N("Reshape", ["x", "rs"], ["a"]),
    N("Transpose", ["a"], ["b"], perm=[1, 0, 2]),
    N("Unsqueeze", ["b", "ax"], ["c"]),
    N("Squeeze", ["c", "ax"], ["d"]),
    N("Identity", ["d"], ["e"]),
    N("Dropout", ["e"], ["y"]),
], (2, 3, 4), inits=[init("rs", np.array([3, 2, -1], dtype=np.int64)),
                      init("ax", np.array([0], dtype=np.int64))])
case("concat_gather", [
    N("Concat", ["x", "x2"], ["a"], axis=1),
    N("Gather", ["a", "idx"], ["b"], axis=1),
    N("Flatten", ["b"], ["y"], axis=1),
], (2, 3, 2), inits=[init("x2", rnd(2, 2, 2)), init("idx", np.array([[0, 4], [3, -1]], dtype=np.int64))])
case("reduce", [
    N("ReduceMean", ["x"], ["a"], axes=[1], keepdims=1),
    N("ReduceMax", ["a"], ["b"], axes=[2], keepdims=0),
    N("ReduceSum", ["x", "ax"], ["c"], keepdims=0),
    N("Add", ["b", "c"], ["y"]),
], (2, 3, 4), inits=[init("ax", np.array([1, 2], dtype=np.int64))])
case("reduce_all", [N("ReduceSum", ["x"], ["y"], keepdims=0)], (2, 3))
case("shape_cast", [
    N("Shape", ["x"], ["s"]),
    N("Cast", ["s"], ["sf"], to=F),
    N("Constant", [], ["k"], value=numpy_helper.from_array(np.array([10.0, 100.0], dtype=np.float32))),
    N("Mul", ["sf", "k"], ["y"]),
], (4, 2))

# --- Softmax antes y después del opset 13
case("softmax13", [N("Softmax", ["x"], ["y"], axis=1)], (2, 3, 4), opset=13)
case("softmax11", [N("Softmax", ["x"], ["y"], axis=1)], (2, 3, 4), opset=11)
case("logsoftmax", [N("LogSoftmax", ["x"], ["y"], axis=-1)], (3, 6), opset=13)
case("clip_attr", [N("Clip", ["x"], ["y"], min=-0.2, max=0.3)], (3, 5), opset=6)

# --- Red tipo MNIST (1x28x28): Conv5 + Relu + MaxPool + Conv5 + Relu + MaxPool + Flatten + Gemm + Softmax
case("mnist", [
    N("Conv", ["x", "w1", "b1"], ["c1"], kernel_shape=[5, 5]),
    N("Relu", ["c1"], ["r1"]),
    N("MaxPool", ["r1"], ["p1"], kernel_shape=[2, 2], strides=[2, 2]),
    N("Conv", ["p1", "w2", "b2"], ["c2"], kernel_shape=[5, 5]),
    N("Relu", ["c2"], ["r2"]),
    N("MaxPool", ["r2"], ["p2"], kernel_shape=[2, 2], strides=[2, 2]),
    N("Flatten", ["p2"], ["f"]),
    N("Gemm", ["f", "fw", "fb"], ["g"]),
    N("Softmax", ["g"], ["y"], axis=1),
], (1, 1, 28, 28), inits=[
    init("w1", rnd(8, 1, 5, 5, scale=0.3)), init("b1", rnd(8, scale=0.1)),
    init("w2", rnd(16, 8, 5, 5, scale=0.1)), init("b2", rnd(16, scale=0.1)),
    init("fw", rnd(256, 10, scale=0.1)), init("fb", rnd(10, scale=0.1))])

# --- Entrada INT64 (run_ids): embedding por Gather + media + Gemm
case("embed_ids", [
    N("Gather", ["emb", "x"], ["e"], axis=0),
    N("ReduceMean", ["e"], ["m"], axes=[1], keepdims=0),
    N("Gemm", ["m", "w", "b"], ["y"]),
], (2, 5), inits=[init("emb", rnd(20, 6)), init("w", rnd(6, 3)), init("b", rnd(3))],
     x=rng.integers(0, 20, size=(2, 5)).astype(np.int64), elem_in=I64)

# --- Operadores de transformers (comparaciones, lógica, Where, Expand, ConstantOfShape, ...)
I = lambda name, v: init(name, np.asarray(v, dtype=np.int64))
case("op_where", [
    N("Greater", ["x", "zero"], ["b"]),
    N("Where", ["b", "x", "neg"], ["y"]),
], (2, 3, 4), inits=[init("zero", np.float32(0.25)), init("neg", rnd(1, 4))])

case("op_logic", [
    N("Less", ["x", "hi"], ["a"]),
    N("Greater", ["x", "lo"], ["b"]),
    N("And", ["a", "b"], ["c"]),
    N("Equal", ["x", "x"], ["e"]),
    N("Xor", ["c", "e"], ["xr"]),
    N("Or", ["xr", "c"], ["o"]),
    N("Not", ["o"], ["n"]),
    N("Cast", ["n"], ["nf"], to=F),
    N("Cast", ["c"], ["cf"], to=F),
    N("Add", ["nf", "cf"], ["y"]),
], (3, 5), inits=[init("hi", np.float32(0.7)), init("lo", np.float32(-0.4))])

case("op_expand", [
    N("Expand", ["x", "shp"], ["e"]),
    N("ConstantOfShape", ["shp"], ["c"], value=helper.make_tensor("v", F, [1], [0.5])),
    N("Add", ["e", "c"], ["y"]),
], (2, 1, 4), inits=[I("shp", [2, 3, 4])])

case("op_slice_range", [
    N("Slice", ["x", "st", "en", "ax", "sp"], ["s"]),
    N("Slice", ["x", "st2", "en2", "ax2", "sp2"], ["r"]),
    N("Range", ["r0", "r1", "r2"], ["rg"]),
    N("Cast", ["rg"], ["rgf"], to=F),
    N("Mul", ["s", "rgf"], ["m"]),
    N("ReduceSum", ["r"], ["rs"], keepdims=0),
    N("Add", ["m", "rs"], ["y"]),
], (2, 8, 3), inits=[I("st", [1, -1]), I("en", [7, -9]), I("ax", [1, 2]), I("sp", [2, -1]),
                     I("st2", [0]), I("en2", [100]), I("ax2", [2]), I("sp2", [1]),
                     I("r0", 1), I("r1", 10), I("r2", 3)],
     x=rnd(2, 8, 3))

case("op_gather_nd", [
    N("GatherElements", ["x", "ie"], ["ge"], axis=1),
    N("GatherND", ["x", "in0"], ["g0"]),
    N("GatherND", ["x", "in1"], ["g1"], batch_dims=1),
    N("ReduceSum", ["ge"], ["s1"], keepdims=0),
    N("ReduceSum", ["g0"], ["s2"], keepdims=0),
    N("ReduceSum", ["g1"], ["s3"], keepdims=0),
    N("Add", ["s1", "s2"], ["s12"]),
    N("Add", ["s12", "s3"], ["y"]),
], (2, 4, 3), inits=[I("ie", [[[0, 3, 1], [2, 2, 0]], [[1, 1, 1], [3, 0, 2]]]),
                     I("in0", [[0, 1], [1, 3]]), I("in1", [[[2], [0]], [[1], [3]]])])

case("op_split_minmax", [
    N("Split", ["x", "sizes"], ["a", "b", "c"], axis=1),
    N("Max", ["a", "b"], ["m1"]),
    N("Min", ["m1", "c", "k"], ["y"]),
], (2, 6, 3), inits=[I("sizes", [2, 2, 2]), init("k", np.float32(0.3))], opset=13)

case("op_erf_gelu", [
    N("Mul", ["x", "three"], ["x3"]),
    N("Erf", ["x3"], ["e"]),
    N("Gelu", ["x3"], ["g"]),
    N("Add", ["e", "g"], ["y"]),
], (4, 9), inits=[init("three", np.float32(3.0))], opset=20)

case("op_layernorm", [
    N("LayerNormalization", ["x", "sc", "bi"], ["l1"], axis=-1, epsilon=1e-5),
    N("LayerNormalization", ["l1", "sc2"], ["y"], axis=1, epsilon=1e-3),
], (3, 4, 6), inits=[init("sc", rnd(6)), init("bi", rnd(6)), init("sc2", rnd(4, 6))], opset=17)

case("op_pow_unary", [
    N("Abs", ["x"], ["ax"]),
    N("Add", ["ax", "one"], ["p"]),
    N("Pow", ["p", "e25"], ["q"]),
    N("Pow", ["x", "e3"], ["c"]),
    N("Reciprocal", ["p"], ["rc"]),
    N("Floor", ["x"], ["fl"]),
    N("Ceil", ["x"], ["ce"]),
    N("Add", ["q", "c"], ["a1"]),
    N("Add", ["rc", "fl"], ["a2"]),
    N("Add", ["a1", "a2"], ["a3"]),
    N("Add", ["a3", "ce"], ["y"]),
], (3, 5), inits=[init("one", np.float32(1.0)), init("e25", np.float32(2.5)), init("e3", np.float32(3.0))])

case("op_reduce_misc", [
    N("ReduceMin", ["x"], ["a"], axes=[1], keepdims=1),
    N("ReduceProd", ["x"], ["b"], axes=[2], keepdims=1),
    N("ReduceL2", ["x"], ["c"], axes=[1, 2], keepdims=1),
    N("ReduceSumSquare", ["x"], ["d"], axes=[0], keepdims=1),
    N("Add", ["a", "b"], ["ab"]),
    N("Add", ["c", "d"], ["cd"]),
    N("Add", ["ab", "cd"], ["y"]),
], (3, 4, 4))

# --- Archivos inválidos (sin .json: los usa bad.titan, no compare.py)
raw = open(f"{OUT}/mlp.onnx", "rb").read()
open(f"{OUT}/bad_trunc.onnx", "wb").write(raw[: len(raw) // 2])
open(f"{OUT}/bad_empty.onnx", "wb").write(b"")
open(f"{OUT}/bad_junk.onnx", "wb").write(bytes(rng.integers(0, 256, size=300, dtype=np.uint8)))
unsup = helper.make_model(helper.make_graph(
    [N("NonMaxSuppression", ["x"], ["y"])], "unsup", [vi("x", [1, 4])], [vi("y", [1, 4])]),
    opset_imports=[helper.make_opsetid("", 13)])
onnx.save(unsup, f"{OUT}/bad_op.onnx")
print("bad_*: trunc, empty, junk, op")
