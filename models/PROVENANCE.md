# Vendored model provenance — `src/vector.rs`

The `embed` cargo feature compiles these bytes into the binary with
`include_bytes!`. They are the reason the vector arm needs no network, no cache
directory, and no file on disk at runtime, and they are the reason the arm's
licence obligations live in the source rather than in a commit message.

**Licence: Apache-2.0.** Compatible with this crate's own `MIT OR Apache-2.0`.
Apache-2.0 §4 requires retaining attribution notices; the licence text and
notices of the upstream project travel in the repository of record linked below.

| file | bytes | sha256 |
|---|---|---|
| `model_int8.onnx` | 22,972,370 | `afdb6f1a0e45b715d0bb9b11772f032c399babd23bfc31fed1c170afc848bdb1` |
| `tokenizer.json` | 711,661 | `da0e79933b9ed51798a3ae27893d3c5fa4a201126cef75586296df9b4d2c62a0` |
| `config.json` | 650 | `7135149f7cffa1a573466c6e4d8423ed73b62fd2332c575bf738a0d033f70df7` |
| `special_tokens_map.json` | 125 | `b6d346be366a7d1d48332dbc9fdf3bf8960b5d879522b7799ddba59e76237ee3` |
| `tokenizer_config.json` | 366 | `9261e7d79b44c8195c1cada2b453e55b00aeb81e907a6664974b4d7776172ab3` |

Source: <https://huggingface.co/Xenova/all-MiniLM-L6-v2> — the ONNX export of
`sentence-transformers/all-MiniLM-L6-v2`. `license: apache-2.0` on the model
card; verified against the Hugging Face model API on 2026-09-28.

Re-verify after any change to these files:

```sh
cd models && sha256sum -c <<'EOF'
afdb6f1a0e45b715d0bb9b11772f032c399babd23bfc31fed1c170afc848bdb1  model_int8.onnx
da0e79933b9ed51798a3ae27893d3c5fa4a201126cef75586296df9b4d2c62a0  tokenizer.json
7135149f7cffa1a573466c6e4d8423ed73b62fd2332c575bf738a0d033f70df7  config.json
b6d346be366a7d1d48332dbc9fdf3bf8960b5d879522b7799ddba59e76237ee3  special_tokens_map.json
9261e7d79b44c8195c1cada2b453e55b00aeb81e907a6664974b4d7776172ab3  tokenizer_config.json
EOF
```

`the_vendored_model_is_the_file_the_provenance_records` in `src/vector.rs` hashes
the bytes the *compiler* put in the binary against the same digest, so a blob
swapped in without updating this file fails the test run rather than passing
quietly.

## Why these five and not `vocab.txt`

The brief for this work asked for `vocab.txt` (231,508 B). The pinned runtime
(`fastembed` 7.1.0) loads its tokenizer with
`tokenizers::Tokenizer::from_bytes`, which is the fast-tokenizers serialisation —
its own error string for that field is literally `"Could not read
tokenizer.json"` (`fastembed/src/common.rs:127`). A `vocab.txt` alone does not
build.

`tokenizer.json` is a strict superset rather than a substitute: its
`model.vocab` holds **byte-identical token→id pairs for all 30,522 entries** of
`vocab.txt` (verified 30,522 vs 30,522, zero differing entries), plus the
normalizer and pre-tokenizer configuration that `vocab.txt` has no room for. The
vocabulary the brief asked for is present, in the form the runtime reads;
`vocab.txt` is not vendored, so the repository does not carry 231 KB of
never-read bytes.

## Why int8 and not fp32

`onnx/model.onnx` is 90.4 MB; `onnx/model_int8.onnx` is 23.0 MB. The int8
dynamically-quantised build is the one vendored. The trade accepted for this
feature was binary size for self-containment, not binary size for unmeasured
precision.
