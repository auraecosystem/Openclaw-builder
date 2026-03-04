# Methods Requiring ML Frameworks

These method families from the cross-disciplinary precursor detection survey require deep learning frameworks (PyTorch, JAX, etc.) and are NOT feasible as pure Rust indicators. Listed here for completeness.

## Deep TSAD via Transformers
Attention discrepancy, reconstruction/prediction, variable-temporal attention. Requires transformer architecture with backprop training.

## Deep TSAD with Diffusion Models
Coarse-to-fine, physically-guided, transformer-diffusion hybrids. Requires score-matching objectives and Gaussian process priors.

## Self-Supervised Representation Learning
Contrastive (NT-Xent), masked, seasonal-trend disentangling. Requires encoder architectures with learned invariances.

## Long-Context Sequence Backbones
Patch-based Transformers, selective state-space models (Mamba/S4). Requires trainable projection layers.

## Time-Series Foundation Models
Pretrained tokenization/quantization, MoE designs. Requires large pretrained models.

## Graph Neural Networks (GNN4TS)
Spatiotemporal graphs, message passing, learnable aggregation. Requires graph convolution layers.

## Shapelets / ROCKET-Family
Random convolutional kernels or learned shapelet networks. Training phase requires backprop.

## Evolutionary NAS for TSAD
Neural architecture search over TS anomaly detection architectures. Compute-heavy search.

## Boundary-Aware Tokenization / Transformers
Boundary-aware tokenization for regime changes. Developed for video/action streams.

## Implementation Path
These methods would be implemented as Python modules using PyTorch/JAX, potentially wrapped via pyo3 bridge (like the `kronos.rs` stub). They are NOT candidates for the Rust indicator crate.
