//! Integer-LLM Kernels
//! 
//! Strenge Fixed-Point-Operationen fuer bit-exakte LLM-Inferenz.
//! Kein Float im Hot-Path.

pub mod backend;
pub mod backends;
pub mod dot;
pub mod fixed_point;
pub mod integer_math;
pub mod konformitaet;
pub mod optimierer;
pub mod prng;
pub mod rechenpfad;
pub mod rmsnorm;
pub mod linear;
pub mod rope;
pub mod softmax;
pub mod attention;
pub mod backward;
pub mod mlp;
pub mod moe;
pub mod sampling;
