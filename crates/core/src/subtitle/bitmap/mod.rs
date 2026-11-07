//! The bitmap subtitle pipeline of design part 10.1, as pure functions of bytes (R-A5): the Matroska reader of
//! evidence.mks (`mkv`), the decoders, the preparation and the row line finder. Nothing here reads a file, a process
//! or a clock (RP19).

pub mod mkv;

/// The two bitmap subtitle codecs the worker reads (design part 10.3); any other codec is refused by the caller
/// (T5-R11). Def: T5-3. Use: T5-4, T5-8, T5-11.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubCodec {
    Pgs,
    VobSub,
}

/// Matroska's codec ID of PGS (Presentation Graphic Stream) subtitles (design part 10.3). Def: T5-3. Use: T5-8,
/// T5-15.
pub const CODEC_PGS: &str = "S_HDMV/PGS";

/// Matroska's codec ID of VobSub (the DVD, Digital Versatile Disc, bitmap format) subtitles (design part 10.3). Def:
/// T5-3. Use: T5-8, T5-15.
pub const CODEC_VOBSUB: &str = "S_VOBSUB";
