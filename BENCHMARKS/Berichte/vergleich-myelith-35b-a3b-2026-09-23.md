# Vergleich: I-LLM und I-ViT

## I-LLM

**Einreichungsdatum:** 28. Mai 2024

**Kernidee:** I-LLM ist ein neuartiges, vollständig quantisiertes PTQ-Framework (Post-Training Quantization) für Large Language Models, das ausschließlich auf ganzzahliger Arithmetik basiert und dabei Floating-Point-Operationen vollständig eliminiert. Die Autoren adressieren die große Fluktuation von Aktivierungen über Kanäle und Tokens hinweg mit Fully-Smooth Block-Reconstruction (FSBR) und Dynamic Integer-only MatMul (DI-MatMul) sowie effizienten ganzzahligen Approximationen für nichtlineare Operatoren wie Softmax und Normalization.

## I-ViT

**Einreichungsdatum:** 4. Juli 2022

**Kernidee:** I-ViT ist ein Integer-Only-Quantisierungsschema für Vision Transformers (ViTs), das den gesamten Inferenz-Computational Graph mit ganzzahliger Arithmetik und Bit-Shifting ausführt, ohne jegliche Floating-Point-Operationen. Lineare Operationen folgen einem Integer-Only-Pipeline mit dyadischer Arithmetik, während nichtlineare Operationen wie Softmax, GELU und LayerNorm durch die vorgeschlagenen leichten ganzzahligen Approximationen Shiftmax und ShiftGELU ersetzt werden.

## Quellen

- https://arxiv.org/abs/2405.17849
- https://arxiv.org/abs/2207.01405
