//! The audio content rule of spec 8.6 (experiment 13), ported from scripts/phase0/audio_similarity.py: `levels` reads
//! a level folder written by audio-levels.ps1, `measures` measures a candidate against the main track. Later tasks add
//! the rule, the reference, the titles listing, the report, the labels and the dataset harness here.

pub mod levels;
pub mod measures;
pub mod reference;
pub mod rule;
pub mod titles;
pub mod report;
