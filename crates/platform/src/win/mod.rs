//! The Windows implementations of lazymkv-platform's traits and helpers (design parts 4.2, 4.3). Every `windows-sys`
//! call of the workspace sits in this module, and each `unsafe` block carries a `// SAFETY:` line saying why the call
//! is sound. The rules these implementations apply (descriptor strings, the token verdict, the share and volume rules,
//! the drive diff, the local-time text, the eject report) sit in modules that compile and are tested on every system.
//! Compiled only on Windows.
